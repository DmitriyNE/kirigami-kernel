//! The CGAL `Arrangement_2` differential harness (feature `cgal`, test-only).
//!
//! Generate an arrangement of lines + circles, run it through both our `arrange2d`
//! pipeline (decompose → arrange_events → touch vertices) and the CGAL
//! circular-kernel `Arrangement_2` oracle, and assert the intersection-vertex
//! point sets agree **exactly** — compared by radical-safe `Surd::cmp`, no
//! tolerance — up to the quotient. Coincident carriers (`SharedCarrier`, our 0
//! events, vs CGAL overlap edges) are excluded from the generator and validated
//! in-crate; here every generated arrangement has distinct carriers, so the CGAL
//! degree-≥3 vertices (genuine multi-curve intersections) match our touch set
//! one-for-one.
//!
//! Lines are bounded to the same wide segment on both sides, so the two engines
//! see identical geometry; the segment is far wider than any small-coordinate
//! intersection.

use crate::cgal::cgal_arrange;
use arrange2d::decompose::decompose;
use arrange2d::spine::arrange_events;
use certify_core::Verdict;
use geom::content::{Circle, Curve, CurveId, Line, Orient, Point2, SegPiece};
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
        Verdict::Verified((set, _)) => set.vertices.into_iter().map(|v| v.point).collect(),
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

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Our arrangement's touch-vertex set equals CGAL's intersection-vertex set
        /// over random line/circle arrangements — exact `a+b√d`, no tolerance.
        #[test]
        fn arrangement_matches_cgal(gens in gen_arrangement()) {
            prop_assert!(same_points(&our_vertices(&gens), &cgal_vertices(&gens)));
        }
    }
}
