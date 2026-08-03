//! The CGAL `Arrangement_2` differential harness (feature `cgal`, test-only).
//!
//! Generate an arrangement of lines + circles, run it through both our `arrange2d`
//! pipeline (decompose → arrange_events → touch vertices) and the CGAL
//! circular-kernel `Arrangement_2` oracle, and assert the intersection-vertex
//! point sets agree **exactly** — compared by radical-safe `Surd::cmp`, no
//! tolerance — up to the quotient. The transverse harness excludes coincident
//! carriers (so the CGAL degree-≥3 vertices match our touch set one-for-one);
//! the **coincidence** harness (slice 3c) handles them directly against the CGAL
//! `Arr_curve_data_traits_2` **overlap-edge** oracle — our merged edge (both
//! operands) ≡ CGAL's overlap edge (covering count ≥ 2), our residuals ≡ CGAL's
//! single-count edges.
//!
//! Lines are bounded to the same wide segment on both sides, so the two engines
//! see identical geometry; the segment is far wider than any small-coordinate
//! intersection.

use crate::cgal::{
    cgal_arrange, cgal_arrange_edges, cgal_boolean_boundary, cgal_boolean_count, cgal_boolean_holes,
};
use arrange2d::decompose::decompose;
use arrange2d::event::{CoincEdge, Operand};
use arrange2d::spine::arrange_events;
use certify_core::Verdict;
use geom::content::{Circle, Curve, CurveId, Edge, Line, Orient, Point2, SegPiece};
use lattice::{Bignum, Rat, Surd};

type Q = Rat<Bignum>;
type P = Point2<Bignum>;

fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// An input curve as raw integers, so the CGAL string and the `arrange2d` value
/// are built from one source (no need to read back the `pub(crate)` `Surd`/`Rat`
/// internals).
#[derive(Clone, Debug)]
enum Gen {
    Line { a: i128, b: i128, c: i128 },
    Circle { cx: i128, cy: i128, r2: i128 },
}

/// Normalise a rational so the denominator is positive.
fn norm(n: i128, d: i128) -> (i128, i128) {
    if d < 0 { (-n, -d) } else { (n, d) }
}

/// The two rational endpoints (`x1,y1,x2,y2` as num/den) of the wide segment a
/// line is bounded to — identical for CGAL and `arrange2d`.
fn seg_pts(a: i128, b: i128, c: i128) -> [(i128, i128); 4] {
    const T: i128 = 1000;
    if b != 0 {
        // y = −(a·x + c)/b, sampled at x = ∓T
        [(-T, 1), norm(a * T - c, b), (T, 1), norm(-(a * T + c), b)]
    } else {
        // vertical line x = −c/a, sampled at y = ∓T
        let x = norm(-c, a);
        [x, (-T, 1), x, (T, 1)]
    }
}

/// The curve as a CGAL input line (`S …` segment / `C …` circle, rationals num/den).
fn cgal_curve(g: &Gen) -> String {
    match g {
        Gen::Line { a, b, c } => {
            let p = seg_pts(*a, *b, *c);
            format!(
                "S {}/{} {}/{} {}/{} {}/{}",
                p[0].0, p[0].1, p[1].0, p[1].1, p[2].0, p[2].1, p[3].0, p[3].1
            )
        }
        Gen::Circle { cx, cy, r2 } => format!("C {cx}/1 {cy}/1 {r2}/1"),
    }
}

/// The curve as an `arrange2d` input `Curve` (same bounded segment / full circle).
fn arr_curve(g: &Gen, src: u32) -> Curve<Bignum> {
    match g {
        Gen::Line { a, b, c } => {
            let p = seg_pts(*a, *b, *c);
            Curve::Seg(SegPiece {
                line: Line {
                    a: qi(*a),
                    b: qi(*b),
                    c: qi(*c),
                },
                start: Point2::from_rat(Q::new(p[0].0, p[0].1), Q::new(p[1].0, p[1].1)),
                end: Point2::from_rat(Q::new(p[2].0, p[2].1), Q::new(p[3].0, p[3].1)),
                orient: Orient::Ccw,
                source: CurveId(src),
            })
        }
        Gen::Circle { cx, cy, r2 } => Curve::Circle {
            circle: Circle {
                cx: qi(*cx),
                cy: qi(*cy),
                r2: qi(*r2),
            },
            orient: Orient::Ccw,
            source: CurveId(src),
        },
    }
}

/// Our arrangement's touch-vertex points.
fn our_vertices(gens: &[Gen]) -> Vec<P> {
    let mut edges = Vec::new();
    for (i, g) in gens.iter().enumerate() {
        edges.extend(decompose(&arr_curve(g, i as u32)));
    }
    match arrange_events(&edges) {
        Verdict::Verified((set, _, _)) => set.vertices.into_iter().map(|v| v.point).collect(),
        _ => unreachable!("degree-≤2 arrangement is always Verified"),
    }
}

/// Parse a rational `num/den` (or a bare integer) into a `Rat`.
fn parse_q(s: &str) -> Q {
    match s.split_once('/') {
        Some((n, d)) => Q::new(n.parse().unwrap(), d.parse().unwrap()),
        None => Q::from_i128(s.parse().unwrap()),
    }
}

/// The CGAL oracle's intersection-vertex points (degree ≥ 3), parsed from the
/// `xa xb xd ya yb yd` triples into `Surd` coordinates.
fn cgal_vertices(gens: &[Gen]) -> Vec<P> {
    let input = gens.iter().map(cgal_curve).collect::<Vec<_>>().join("\n");
    cgal_arrange(&input)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            Point2 {
                x: Surd::new(parse_q(t[0]), parse_q(t[1]), parse_q(t[2])),
                y: Surd::new(parse_q(t[3]), parse_q(t[4]), parse_q(t[5])),
            }
        })
        .collect()
}

/// Both vertex sets are deduped, so equal length + one-way containment is set
/// equality (points compared by value via `Point2`'s radical-safe `Eq`).
fn same_points(a: &[P], b: &[P]) -> bool {
    a.len() == b.len() && a.iter().all(|p| b.contains(p))
}

// --- coincidence (overlap-edge) differential ---

/// A canonical edge key: the two endpoints (ordered) + the number of covering
/// curves (our `Both` ⇒ 2, residual ⇒ 1; CGAL's popcount).
fn edge_key(p: P, q: P, count: u32) -> (P, P, u32) {
    if p <= q { (p, q, count) } else { (q, p, count) }
}

/// Our stage-2 coincidence output edges as canonical keys.
fn our_coinc_keys(edges: &[Edge<Bignum>]) -> Vec<(P, P, u32)> {
    let coinc = match arrange_events(edges) {
        Verdict::Verified((_, c, _)) => c,
        _ => unreachable!(),
    };
    coinc
        .iter()
        .map(|e: &CoincEdge<Bignum>| {
            let (s, t) = match &e.edge {
                Edge::Seg(sp) => (sp.start.clone(), sp.end.clone()),
                Edge::Arc(a) => (a.start.clone(), a.end.clone()),
            };
            edge_key(s, t, if e.operand == Operand::Both { 2 } else { 1 })
        })
        .collect()
}

/// CGAL's arrangement edges as canonical keys (from `cgal_arrange_edges`).
fn cgal_edge_keys(input: &str) -> Vec<(P, P, u32)> {
    cgal_arrange_edges(input)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            let count: u32 = t[0].parse().unwrap();
            let p = Point2 {
                x: Surd::new(parse_q(t[1]), parse_q(t[2]), parse_q(t[3])),
                y: Surd::new(parse_q(t[4]), parse_q(t[5]), parse_q(t[6])),
            };
            let q = Point2 {
                x: Surd::new(parse_q(t[7]), parse_q(t[8]), parse_q(t[9])),
                y: Surd::new(parse_q(t[10]), parse_q(t[11]), parse_q(t[12])),
            };
            edge_key(p, q, count)
        })
        .collect()
}

/// Two coincident segments through `base + t·dir`, and the matching CGAL input
/// (curve ids 0, 1). Endpoints are integers, so both engines see identical
/// geometry and the two segments share the exact same carrier line.
fn coincident_segments(
    bx: i128,
    by: i128,
    dx: i128,
    dy: i128,
    t: [i128; 4],
) -> (Vec<Edge<Bignum>>, String) {
    let pt = |tt: i128| (bx + tt * dx, by + tt * dy);
    // line through the direction (dx, dy): normal (dy, −dx), c = −(dy·bx − dx·by).
    let line = Line {
        a: qi(dy),
        b: qi(-dx),
        c: qi(-(dy * bx - dx * by)),
    };
    let seg = |ta: i128, tb: i128, src: u32| {
        let (x0, y0) = pt(ta);
        let (x1, y1) = pt(tb);
        Edge::Seg(Box::new(SegPiece {
            line: line.clone(),
            start: Point2::from_rat(qi(x0), qi(y0)),
            end: Point2::from_rat(qi(x1), qi(y1)),
            orient: Orient::Ccw,
            source: CurveId(src),
        }))
    };
    let edges = vec![seg(t[0], t[1], 0), seg(t[2], t[3], 1)];
    let (x0, y0) = pt(t[0]);
    let (x1, y1) = pt(t[1]);
    let (x2, y2) = pt(t[2]);
    let (x3, y3) = pt(t[3]);
    let cgal = format!("S {x0}/1 {y0}/1 {x1}/1 {y1}/1 0\nS {x2}/1 {y2}/1 {x3}/1 {y3}/1 1");
    (edges, cgal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn coincident(x: &Gen, y: &Gen) -> bool {
        match (x, y) {
            (
                Gen::Line {
                    a: a1,
                    b: b1,
                    c: c1,
                },
                Gen::Line {
                    a: a2,
                    b: b2,
                    c: c2,
                },
            ) => a1 * b2 == a2 * b1 && a1 * c2 == a2 * c1 && b1 * c2 == b2 * c1,
            (
                Gen::Circle {
                    cx: x1,
                    cy: y1,
                    r2: r1,
                },
                Gen::Circle {
                    cx: x2,
                    cy: y2,
                    r2: r2b,
                },
            ) => x1 == x2 && y1 == y2 && r1 == r2b,
            _ => false,
        }
    }

    fn one_gen() -> impl Strategy<Value = Gen> {
        prop_oneof![
            (-6i128..=6, -6i128..=6, -8i128..=8)
                .prop_filter("real line", |&(a, b, _)| a != 0 || b != 0)
                .prop_map(|(a, b, c)| Gen::Line { a, b, c }),
            (-5i128..=5, -5i128..=5, 1i128..=20).prop_map(|(cx, cy, r2)| Gen::Circle {
                cx,
                cy,
                r2
            }),
        ]
    }

    fn gen_arrangement() -> impl Strategy<Value = Vec<Gen>> {
        proptest::collection::vec(one_gen(), 2..=3).prop_filter("distinct carriers", |gs| {
            (0..gs.len()).all(|i| ((i + 1)..gs.len()).all(|j| !coincident(&gs[i], &gs[j])))
        })
    }

    /// Deterministic corpus configs, then the randomized differential.
    #[test]
    fn differential_units() {
        let cases: &[Vec<Gen>] = &[
            // two transverse lines
            vec![
                Gen::Line { a: 0, b: 1, c: 0 },
                Gen::Line { a: 1, b: 0, c: 0 },
            ],
            // line secant through a circle
            vec![
                Gen::Line { a: 0, b: 1, c: 0 },
                Gen::Circle {
                    cx: 0,
                    cy: 0,
                    r2: 2,
                },
            ],
            // line tangent to a circle
            vec![
                Gen::Line { a: 0, b: 1, c: -1 },
                Gen::Circle {
                    cx: 0,
                    cy: 0,
                    r2: 1,
                },
            ],
            // two circles meeting in two points
            vec![
                Gen::Circle {
                    cx: 0,
                    cy: 0,
                    r2: 1,
                },
                Gen::Circle {
                    cx: 1,
                    cy: 0,
                    r2: 1,
                },
            ],
            // externally tangent circles
            vec![
                Gen::Circle {
                    cx: 0,
                    cy: 0,
                    r2: 1,
                },
                Gen::Circle {
                    cx: 2,
                    cy: 0,
                    r2: 1,
                },
            ],
            // three concurrent lines
            vec![
                Gen::Line { a: 0, b: 1, c: 0 },
                Gen::Line { a: 1, b: 0, c: 0 },
                Gen::Line { a: 1, b: -1, c: 0 },
            ],
            // disjoint circles (no intersection)
            vec![
                Gen::Circle {
                    cx: 0,
                    cy: 0,
                    r2: 1,
                },
                Gen::Circle {
                    cx: 5,
                    cy: 0,
                    r2: 1,
                },
            ],
        ];
        for gens in cases {
            assert!(
                same_points(&our_vertices(gens), &cgal_vertices(gens)),
                "mismatch on {gens:?}: ours={:?} cgal={:?}",
                our_vertices(gens),
                cgal_vertices(gens),
            );
        }
    }

    /// Coincidence corpus: two overlapping collinear segments — our stage-2
    /// merged + residual edges match CGAL's overlap (count 2) + single (count 1)
    /// edges.
    #[test]
    fn coincident_units() {
        // (base, dir, [t0,t1,t2,t3]): partial / containment / equality / shared-end.
        let cases: &[(i128, i128, i128, i128, [i128; 4])] = &[
            (0, 0, 1, 0, [0, 4, 2, 6]),  // horizontal partial
            (0, 0, 1, 0, [0, 6, 2, 4]),  // horizontal containment
            (0, 0, 1, 0, [0, 4, 0, 4]),  // equality
            (0, 0, 1, 1, [0, 4, 2, 6]),  // diagonal partial
            (1, -2, 0, 1, [0, 5, 2, 7]), // vertical partial
            (0, 0, 2, 1, [-3, 3, 0, 5]), // diagonal, shifted
        ];
        for &(bx, by, dx, dy, t) in cases {
            let (edges, cgal) = coincident_segments(bx, by, dx, dy, t);
            let mut ours = our_coinc_keys(&edges);
            let mut theirs = cgal_edge_keys(&cgal);
            ours.sort();
            theirs.sort();
            assert_eq!(
                ours,
                theirs,
                "coincident mismatch at {:?}",
                (bx, by, dx, dy, t)
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Our arrangement's touch-vertex set equals CGAL's intersection-vertex set
        /// over random line/circle arrangements — exact `a+b√d`, no tolerance.
        #[test]
        fn arrangement_matches_cgal(gens in gen_arrangement()) {
            prop_assert!(same_points(&our_vertices(&gens), &cgal_vertices(&gens)));
        }

        /// Two random OVERLAPPING coincident segments: our merged + residual edges
        /// match CGAL's overlap + single edges exactly (endpoints via Surd::cmp,
        /// covering-count via popcount vs Both/residual).
        #[test]
        fn coincident_edges_match_cgal(
            bx in -3i128..=3, by in -3i128..=3, dx in -3i128..=3, dy in -3i128..=3,
            t0 in -6i128..=6, t1 in -6i128..=6, t2 in -6i128..=6, t3 in -6i128..=6,
        ) {
            prop_assume!(dx != 0 || dy != 0);
            prop_assume!(t0 != t1 && t2 != t3);
            let (sa0, sa1) = (t0.min(t1), t0.max(t1));
            let (sb0, sb1) = (t2.min(t3), t2.max(t3));
            prop_assume!(sa0.max(sb0) < sa1.min(sb1)); // genuine overlap
            let (edges, cgal) = coincident_segments(bx, by, dx, dy, [t0, t1, t2, t3]);
            let mut ours = our_coinc_keys(&edges);
            let mut theirs = cgal_edge_keys(&cgal);
            ours.sort();
            theirs.sort();
            prop_assert_eq!(ours, theirs);
        }
    }

    // --- slice 3d: the boolean region differential (Boolean_set_operations_2) ------

    use arrange2d::boolean::{BoolOp, OperandId, ledge_dom};

    type Disk = (i128, i128, i128);

    /// The CGAL two-operand input for disks `c1` (operand A) and `c2` (operand B).
    fn cgal_bool_input(c1: Disk, c2: Disk) -> String {
        format!(
            "C {}/1 {}/1 {}/1 0\nC {}/1 {}/1 {}/1 1",
            c1.0, c1.1, c1.2, c2.0, c2.1, c2.2
        )
    }
    /// Our emitted π₀ face count of `op` over disk operands `c1` (A) and `c2` (B).
    fn our_faces(c1: Disk, c2: Disk, op: BoolOp) -> usize {
        let mk = |c: Disk, src: u32| {
            decompose(&Curve::Circle {
                circle: Circle {
                    cx: qi(c.0),
                    cy: qi(c.1),
                    r2: qi(c.2),
                },
                orient: Orient::Ccw,
                source: CurveId(src),
            })
        };
        let mut edges = mk(c1, 0);
        edges.extend(mk(c2, 1));
        let operand = |s: CurveId| {
            if s.0 == 0 { OperandId::A } else { OperandId::B }
        };
        ledge_dom(&edges, &operand, op).faces.len()
    }
    fn cgal_count(c1: Disk, c2: Disk, op: &str) -> usize {
        cgal_boolean_count(&cgal_bool_input(c1, c2), op)
            .parse()
            .unwrap()
    }

    /// ∪ and ∩ of properly-overlapping disks are **non-pinching** (a single joined
    /// region / a single lens), so our π₀ emitted face count agrees exactly with
    /// CGAL's independent `Boolean_set_operations_2` component count.
    #[test]
    fn boolean_union_intersection_match_cgal() {
        let pairs: &[(Disk, Disk)] = &[
            ((0, 0, 25), (8, 0, 25)),
            ((0, 0, 4), (1, 0, 4)),
            ((0, 0, 9), (4, 0, 9)),
            ((-1, 0, 16), (3, 1, 16)),
        ];
        for &(c1, c2) in pairs {
            assert_eq!(
                our_faces(c1, c2, BoolOp::And),
                cgal_count(c1, c2, "and"),
                "∩ {c1:?} {c2:?}"
            );
            assert_eq!(
                our_faces(c1, c2, BoolOp::Or),
                cgal_count(c1, c2, "or"),
                "∪ {c1:?} {c2:?}"
            );
        }
    }

    /// △ of overlapping disks is **pinched** at the two crossing points: our π₀
    /// separates it into two lunes (spec §6 — π₀ keeps them separate, CAP-OUT-LINK
    /// rejects the pinch vertex), while CGAL's set-boolean joins them into one
    /// polygon-with-holes. The documented, spec-aligned semantic boundary — the
    /// count differs by exactly the pinch.
    #[test]
    fn boolean_xor_pinch_documented() {
        let (c1, c2) = ((0, 0, 25), (8, 0, 25));
        assert_eq!(our_faces(c1, c2, BoolOp::Xor), 2, "our π₀: two lunes");
        assert_eq!(
            cgal_count(c1, c2, "xor"),
            1,
            "CGAL: one pinch-joined region"
        );
    }

    /// Our total emitted hole count for `op`.
    fn our_holes(c1: Disk, c2: Disk, op: BoolOp) -> usize {
        let mk = |c: Disk, src: u32| {
            decompose(&Curve::Circle {
                circle: Circle {
                    cx: qi(c.0),
                    cy: qi(c.1),
                    r2: qi(c.2),
                },
                orient: Orient::Ccw,
                source: CurveId(src),
            })
        };
        let mut edges = mk(c1, 0);
        edges.extend(mk(c2, 1));
        let operand = |s: CurveId| if s.0 == 0 { OperandId::A } else { OperandId::B };
        ledge_dom(&edges, &operand, op)
            .faces
            .iter()
            .map(|f| f.holes.len())
            .sum()
    }
    fn cgal_holes(c1: Disk, c2: Disk, op: &str) -> usize {
        cgal_boolean_holes(&cgal_bool_input(c1, c2), op)
            .parse()
            .unwrap()
    }

    /// Slice-3e Option-B structural cross-check over the **non-pinching** part of the
    /// full regime: for disjoint and nested operands our emitted (faces, holes) match
    /// CGAL's independent `General_polygon_with_holes_2` (component, hole) counts —
    /// including the **annulus △ = one face with one hole**, the case flat π₀ could not
    /// represent before 3e.1b. (△ of *overlapping* disks stays the documented pinch
    /// divergence and is excluded here.)
    #[test]
    fn boolean_faces_holes_match_cgal() {
        // (c1, c2, op) over disjoint + nested configs — all non-pinching.
        let cases: &[(Disk, Disk, BoolOp, &str)] = &[
            // disjoint disks: ∪ = two faces, ∩ = empty
            ((0, 0, 4), (9, 0, 4), BoolOp::Or, "or"),
            ((0, 0, 4), (9, 0, 4), BoolOp::And, "and"),
            // nested (inner strictly inside outer): ∪ = outer, ∩ = inner, △ = annulus
            ((0, 0, 25), (0, 0, 4), BoolOp::Or, "or"),
            ((0, 0, 25), (0, 0, 4), BoolOp::And, "and"),
            ((0, 0, 25), (0, 0, 4), BoolOp::Xor, "xor"),
        ];
        for &(c1, c2, op, ops) in cases {
            assert_eq!(
                our_faces(c1, c2, op),
                cgal_count(c1, c2, ops),
                "face count {c1:?} {c2:?} {ops}"
            );
            assert_eq!(
                our_holes(c1, c2, op),
                cgal_holes(c1, c2, ops),
                "hole count {c1:?} {c2:?} {ops}"
            );
        }
    }

    /// A point-set deduped by radical-safe `Point2` equality.
    fn dedup_points(v: Vec<P>) -> Vec<P> {
        let mut out: Vec<P> = Vec::new();
        for p in v {
            if !out.contains(&p) {
                out.push(p);
            }
        }
        out
    }
    /// Our boolean output's boundary vertex set (every outer + hole edge endpoint).
    fn our_boundary_verts(c1: Disk, c2: Disk, op: BoolOp) -> Vec<P> {
        let mk = |c: Disk, src: u32| {
            decompose(&Curve::Circle {
                circle: Circle {
                    cx: qi(c.0),
                    cy: qi(c.1),
                    r2: qi(c.2),
                },
                orient: Orient::Ccw,
                source: CurveId(src),
            })
        };
        let mut edges = mk(c1, 0);
        edges.extend(mk(c2, 1));
        let operand = |s: CurveId| if s.0 == 0 { OperandId::A } else { OperandId::B };
        let region = ledge_dom(&edges, &operand, op);
        let mut vs = Vec::new();
        for f in &region.faces {
            for e in f.outer.iter().chain(f.holes.iter().flatten()) {
                let (s, t) = match e {
                    Edge::Seg(s) => (s.start.clone(), s.end.clone()),
                    Edge::Arc(a) => (a.start.clone(), a.end.clone()),
                };
                vs.push(s);
                vs.push(t);
            }
        }
        dedup_points(vs)
    }
    /// CGAL's boolean boundary vertex set, parsed from the `xa xb xd ya yb yd` triples.
    fn cgal_boundary_verts(c1: Disk, c2: Disk, op: &str) -> Vec<P> {
        let out = cgal_boolean_boundary(&cgal_bool_input(c1, c2), op);
        let vs = out
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let t: Vec<&str> = l.split_whitespace().collect();
                Point2 {
                    x: Surd::new(parse_q(t[0]), parse_q(t[1]), parse_q(t[2])),
                    y: Surd::new(parse_q(t[3]), parse_q(t[4]), parse_q(t[5])),
                }
            })
            .collect();
        dedup_points(vs)
    }

    /// **Exact `a+b√d` boundary-geometry differential** (#2): the boolean output's
    /// boundary vertex set matches CGAL's `General_polygon_with_holes_2` boundary
    /// exactly (radical-safe `Surd` equality, no tolerance) — not just face/hole counts.
    /// Covers disjoint + nested (the non-pinching regime) with **rational** (`r²∈{4,25}`)
    /// and **irrational** (`r²∈{2,8}` → `±√2`, `±2√2` extrema) radii, so the `a+b√d`
    /// path is genuinely exercised.
    #[test]
    fn boolean_boundary_geometry_matches_cgal() {
        let cases: &[(Disk, Disk, BoolOp, &str)] = &[
            ((0, 0, 4), (9, 0, 4), BoolOp::Or, "or"), // disjoint, rational extrema
            ((0, 0, 2), (9, 0, 2), BoolOp::Or, "or"), // disjoint, ±√2 extrema
            ((0, 0, 25), (0, 0, 4), BoolOp::And, "and"), // nested ∩ = inner
            ((0, 0, 25), (0, 0, 4), BoolOp::Or, "or"), // nested ∪ = outer
            ((0, 0, 25), (0, 0, 4), BoolOp::Xor, "xor"), // annulus
            ((0, 0, 8), (0, 0, 2), BoolOp::Xor, "xor"), // annulus, ±2√2 / ±√2
        ];
        for &(c1, c2, op, ops) in cases {
            assert!(
                same_points(
                    &our_boundary_verts(c1, c2, op),
                    &cgal_boundary_verts(c1, c2, ops)
                ),
                "boundary vertex geometry {c1:?} {c2:?} {ops}"
            );
        }
    }
}
