//! **Boundary elements, and the rule that lets them close.**
//!
//! Both readers produce the same thing: a bag of segments, arcs and whole circles in target-unit
//! coordinates. Turning that bag into the closed loops a `Profile` needs is where the file's own
//! sloppiness has to be dealt with, and the rule is the interesting part.
//!
//! # Arcs pin, segments follow
//!
//! Adjacent entities in a real file meet only to file precision, and a `Profile` needs them to
//! share a vertex *exactly*. A segment through two rational points is exact wherever those points
//! are, so it can be moved for free; an arc has a consistency condition it would violate. So at
//! every junction the **arc's** endpoint is authoritative and the neighbouring segment's endpoint is
//! moved onto it. The distance moved is recorded as the **closure gap** — the file's number, not
//! ours — and never folded into `δ`, because we did not degrade the arc, we absorbed a gap the file
//! already had.
//!
//! Two arcs meeting is the case where neither side is free: their exact endpoints sit on *different*
//! circles, and neither may move without leaving its own. What **is** free is an arc's radius, so
//! the follower is re-gauged onto the shared vertex instead — [`ExactArc::regauged`], §4.3's rule
//! applied at a junction rather than at an endpoint. Unlike a moved segment this is not free: the
//! emitted arc is a different arc from the one the file stated, so it is charged to `δ`, while the
//! gap it absorbed is still reported as the file's own closure gap. The two answer different
//! questions — *how sloppy was the file* and *how far is our geometry from what it said* — and a
//! junction that a real drawing has always produces both.
//!
//! **This does not bite where it would matter.** A DXF `LWPOLYLINE` of bulge arcs shares every
//! vertex *exactly* by construction ([`crate::arc::from_bulge`]), so a bulge polyline — the common
//! outline form — has no junction problem at all. Only chained `ARC` entities can hit it, and a real
//! device drawing does: `acceptance/data/inner-cut.dxf` is eight `ARC`/`LINE` entities with four
//! arc-to-arc junctions.
//!
//! # Where the free junction is spent
//!
//! A loop closes on itself, so one junction has no follower left to move — the walk's last element
//! must meet the vertex the *first* one already owns. A re-gauge cannot repair that one, because
//! moving the head's start would break the head's own far end. So the walk **seeds at a segment**
//! whenever the loop has one: a segment is the endpoint that costs nothing to move, and spending it
//! on the closure leaves every arc-to-arc junction in the interior, where the lift applies.
//!
//! A loop of nothing but arcs has to be lucky instead, and *concentric* arcs are: re-gauging the
//! follower puts it on the leader's own circle, so its sweep carries it exactly back to the anchor
//! and the lens closes at gap zero. Arcs about different centres come round at their own radius and
//! land beside the anchor — [`ImportFault::ArcJunctionGap`], and the cascade that would repair it is
//! not built until a real file needs it.

use crate::arc::ExactArc;
use crate::num::sqrt_rational;
use crate::report::ImportFault;
use arrange2d::profile::Profile;
use lattice::{Backend, Rat};

/// One boundary element as read, in target-unit coordinates.
#[derive(Debug)]
pub enum Element<B: Backend> {
    /// A straight segment between two exact points.
    Segment {
        /// Start point.
        start: [Rat<B>; 2],
        /// End point.
        end: [Rat<B>; 2],
    },
    /// A circular arc, endpoints exactly on its circle.
    Arc(ExactArc<B>),
    /// A whole circle — a closed loop on its own, with no endpoints to chain.
    Circle {
        /// Centre x.
        cx: Rat<B>,
        /// Centre y.
        cy: Rat<B>,
        /// Squared radius.
        r2: Rat<B>,
    },
}

impl<B: Backend> Clone for Element<B> {
    fn clone(&self) -> Self {
        match self {
            Element::Segment { start, end } => Element::Segment {
                start: [start[0].clone(), start[1].clone()],
                end: [end[0].clone(), end[1].clone()],
            },
            Element::Arc(a) => Element::Arc(a.clone()),
            Element::Circle { cx, cy, r2 } => Element::Circle {
                cx: cx.clone(),
                cy: cy.clone(),
                r2: r2.clone(),
            },
        }
    }
}

impl<B: Backend> Element<B> {
    /// The element's start point, or `None` for a closed circle.
    pub fn start(&self) -> Option<[Rat<B>; 2]> {
        match self {
            Element::Segment { start, .. } => Some([start[0].clone(), start[1].clone()]),
            Element::Arc(a) => Some([a.start[0].clone(), a.start[1].clone()]),
            Element::Circle { .. } => None,
        }
    }

    /// The element's end point, or `None` for a closed circle.
    pub fn end(&self) -> Option<[Rat<B>; 2]> {
        match self {
            Element::Segment { end, .. } => Some([end[0].clone(), end[1].clone()]),
            Element::Arc(a) => Some([a.end[0].clone(), a.end[1].clone()]),
            Element::Circle { .. } => None,
        }
    }

    /// Whether this element's endpoints are pinned to a circle (so they may not be moved).
    pub fn is_arc(&self) -> bool {
        matches!(self, Element::Arc(_))
    }

    /// The same geometry traversed the other way.
    pub fn reversed(self) -> Self {
        match self {
            Element::Segment { start, end } => Element::Segment {
                start: end,
                end: start,
            },
            Element::Arc(a) => Element::Arc(ExactArc {
                start: a.end,
                end: a.start,
                ccw: !a.ccw,
                ..a
            }),
            c @ Element::Circle { .. } => c,
        }
    }

    /// Move the start point (segments only — an arc's endpoints belong to its circle).
    fn move_start(&mut self, p: [Rat<B>; 2]) {
        if let Element::Segment { start, .. } = self {
            *start = p;
        }
    }

    /// The element's certified backward error.
    pub fn delta(&self) -> Rat<B> {
        match self {
            Element::Arc(a) => a.delta.clone(),
            _ => Rat::from_i128(0),
        }
    }
}

/// The closed loops a bag of elements assembled into, and the worst gap that took.
#[derive(Debug)]
pub struct Loops<B: Backend> {
    /// One entry per closed loop, elements head-to-tail.
    pub loops: Vec<Vec<Element<B>>>,
    /// The largest distance between adjacent entities that had to be absorbed — **the file's**
    /// number, never mixed into `δ`.
    pub closure_gap: Rat<B>,
    /// The largest certified backward error over the *assembled* elements — which is not the
    /// largest over the elements that went in, because a junction re-gauge raises an arc's `δ`.
    pub delta: Rat<B>,
}

/// `|p − q|²`, exactly.
fn dist2<B: Backend>(p: &[Rat<B>; 2], q: &[Rat<B>; 2]) -> Rat<B> {
    let dx = p[0].sub(&q[0]);
    let dy = p[1].sub(&q[1]);
    dx.mul(&dx).add(&dy.mul(&dy))
}

/// Chain a bag of elements into closed loops, welding endpoints no further apart than `weld`.
///
/// Whole circles become single-element loops. The rest are chained greedily: from each unused
/// element, the walk takes any unused element with an endpoint within `weld` of the running end,
/// orients it to continue, and snaps the junction per the module's rule. Segments are tried as
/// seeds first, so the one junction that cannot be re-gauged — the closure — falls on the endpoint
/// that is free to move. A chain that runs out before returning to its start is
/// [`ImportFault::OpenLoop`] with the shortfall reported.
pub fn assemble<B: Backend>(
    elements: Vec<Element<B>>,
    weld: &Rat<B>,
) -> Result<Loops<B>, ImportFault> {
    let weld2 = weld.mul(weld);
    let mut gap2 = Rat::from_i128(0);
    let mut loops: Vec<Vec<Element<B>>> = Vec::new();

    let mut open: Vec<Element<B>> = Vec::new();
    for e in elements {
        match e {
            Element::Circle { .. } => loops.push(vec![e]),
            _ => open.push(e),
        }
    }

    // Segments first: see "Where the free junction is spent".
    let seeds: Vec<usize> = (0..open.len())
        .filter(|&i| !open[i].is_arc())
        .chain((0..open.len()).filter(|&i| open[i].is_arc()))
        .collect();

    let mut used = vec![false; open.len()];
    for seed in seeds {
        if used[seed] {
            continue;
        }
        used[seed] = true;
        let mut chain = vec![open[seed].clone()];
        let anchor = open[seed].start().expect("open elements have endpoints");

        loop {
            let tail = chain.last().expect("nonempty").end().expect("open");
            // Closed?
            let back = dist2(&tail, &anchor);
            if back.is_zero() {
                break;
            }
            let next = (0..open.len()).find(|&j| {
                !used[j]
                    && open[j].start().zip(open[j].end()).is_some_and(|(s, e)| {
                        dist2(&s, &tail) <= weld2 || dist2(&e, &tail) <= weld2
                    })
            });
            let Some(j) = next else {
                if back <= weld2 {
                    // Close the loop on the anchor, by the same rule as any other junction.
                    let (tail_is_arc, head_is_arc) =
                        (chain.last().expect("nonempty").is_arc(), chain[0].is_arc());
                    if !tail_is_arc {
                        if let Some(Element::Segment { end, .. }) = chain.last_mut() {
                            *end = anchor.clone();
                        }
                    } else if !head_is_arc {
                        chain[0].move_start(tail.clone());
                    } else if !back.is_zero() {
                        return Err(ImportFault::ArcJunctionGap {
                            gap: gap_text(&back),
                        });
                    }
                    if back > gap2 {
                        gap2 = back;
                    }
                    break;
                }
                return Err(ImportFault::OpenLoop {
                    gap: gap_text(&back),
                    at: format!("({}, {})", gap_text(&tail[0]), gap_text(&tail[1])),
                });
            };

            used[j] = true;
            let forward = dist2(&open[j].start().expect("open"), &tail) <= weld2;
            let mut e = open[j].clone();
            if !forward {
                e = e.reversed();
            }
            let head = e.start().expect("open");
            let d = dist2(&head, &tail);
            if !d.is_zero() {
                let previous_is_arc = chain.last().expect("nonempty").is_arc();
                if !e.is_arc() {
                    e.move_start(tail.clone()); // a segment follows
                } else if !previous_is_arc {
                    if let Some(Element::Segment { end, .. }) = chain.last_mut() {
                        *end = head.clone(); // the arc pins, the previous segment follows
                    }
                } else if let Element::Arc(a) = &e {
                    // Two arcs: neither endpoint may move, so the follower's *radius* does.
                    let Ok(fixed) = a.regauged(tail.clone()) else {
                        return Err(ImportFault::ArcJunctionGap { gap: gap_text(&d) });
                    };
                    e = Element::Arc(fixed);
                }
                if d > gap2 {
                    gap2 = d;
                }
            }
            chain.push(e);
        }
        loops.push(chain);
    }

    if loops.is_empty() {
        return Err(ImportFault::Empty);
    }
    let mut delta = Rat::from_i128(0);
    for chain in &loops {
        for e in chain {
            delta = crate::max_of(delta, e.delta());
        }
    }
    Ok(Loops {
        loops,
        closure_gap: sqrt_rational(&gap2, 32),
        delta,
    })
}

/// A squared quantity as a readable decimal distance (for a refusal message only).
fn gap_text<B: Backend>(d2: &Rat<B>) -> String {
    crate::num::to_decimal(&sqrt_rational(&abs(d2), 32), 9)
}

fn abs<B: Backend>(q: &Rat<B>) -> Rat<B> {
    if q.sign() < 0 { q.neg() } else { q.clone() }
}

/// Turn assembled loops into the arrangement edges a cutter profile consumes.
///
/// Straight runs and arcs go in as themselves — no chording — so a radiused outline stays a
/// radiused outline all the way to the wall equations. Nesting needs no ordering: the profile's
/// fill rule is even-odd, so a loop drawn inside another is a hole.
pub fn to_profile<B: Backend>(loops: &[Vec<Element<B>>]) -> Profile<B> {
    let mut p = Profile::new();
    for chain in loops {
        for e in chain {
            p = match e {
                Element::Segment { start, end } => p.polyline(&[start.clone(), end.clone()]),
                // `Profile::arc` always draws the **counter-clockwise** run from its first point to
                // its second, so a clockwise arc is the same geometry with the endpoints swapped.
                // Handing it a clockwise arc unswapped does not fail — it silently draws the
                // *major* arc the long way round, which decomposes into extra monotone pieces and
                // fills the complement of what was authored.
                Element::Arc(a) => {
                    let (from, to) = if a.ccw {
                        (a.start.clone(), a.end.clone())
                    } else {
                        (a.end.clone(), a.start.clone())
                    };
                    p.arc(a.cx.clone(), a.cy.clone(), a.r2.clone(), from, to)
                }
                Element::Circle { cx, cy, r2 } => p.circle_r2(cx.clone(), cy.clone(), r2.clone()),
            };
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arc::{ArcTolerance, from_bulge, from_centre_angles};
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    fn p(x: i128, y: i128) -> [Q; 2] {
        [Q::from_i128(x), Q::from_i128(y)]
    }

    fn seg(a: [Q; 2], b: [Q; 2]) -> Element<Bignum> {
        Element::Segment { start: a, end: b }
    }

    /// A square drawn in the order a file happens to list it, with two sides reversed — assembly
    /// orients them, and the closure gap is exactly zero because nothing had to move.
    #[test]
    fn a_shuffled_square_assembles_with_no_gap() {
        let bag = vec![
            seg(p(0, 0), p(1, 0)),
            seg(p(1, 1), p(1, 0)), // reversed
            seg(p(1, 1), p(0, 1)),
            seg(p(0, 0), p(0, 1)), // reversed
        ];
        let out = assemble::<Bignum>(bag, &Q::new(1, 1000)).expect("closes");
        assert_eq!(out.loops.len(), 1);
        assert_eq!(out.loops[0].len(), 4);
        assert_eq!(out.closure_gap, Q::from_i128(0), "nothing moved");
        // Head-to-tail, exactly.
        for w in out.loops[0].windows(2) {
            assert_eq!(w[0].end(), w[1].start());
        }
    }

    /// A whole circle is its own loop and never chains.
    #[test]
    fn a_circle_is_its_own_loop() {
        let bag = vec![
            Element::Circle {
                cx: Q::from_i128(0),
                cy: Q::from_i128(0),
                r2: Q::from_i128(4),
            },
            seg(p(0, 0), p(1, 0)),
            seg(p(1, 0), p(0, 0)),
        ];
        let out = assemble::<Bignum>(bag, &Q::new(1, 1000)).expect("closes");
        assert_eq!(out.loops.len(), 2);
        assert!(out.loops.iter().any(|l| l.len() == 1 && !l[0].is_arc()));
    }

    /// **The rule**: at a segment–arc junction the arc's endpoint wins and the segment moves onto
    /// it. The arc must come out of assembly still exactly on its own circle, and the distance the
    /// segment travelled must appear as the *closure gap* rather than as `δ`.
    #[test]
    fn an_arc_pins_the_junction_and_the_segment_follows() {
        // A 60° arc of radius 1 about the origin, from 30° to 90°. 30° is *not* a multiple of a
        // quarter turn, so its endpoint is snapped rather than exact — which is the case this test
        // exists for. (A 0°/90°/180°/270° endpoint is exact, and would make the test vacuous.)
        let arc = from_centre_angles::<Bignum>(
            p(0, 0),
            &Q::from_i128(1),
            &Q::from_i128(30),
            &Q::from_i128(90),
            &ArcTolerance::report_only(),
        )
        .expect("certified");
        let delta = arc.delta.clone();
        assert!(delta.sign() > 0, "30° must be the snapped case");
        // Two segments closing the sector, drawn to the *nominal* corner (cos 30°, sin 30°).
        let nominal = [
            Q::new(8_660_254_037_844_386, 10_000_000_000_000_000),
            Q::new(1, 2),
        ];
        let bag = vec![
            Element::Arc(arc),
            seg(p(0, 1), p(0, 0)),
            Element::Segment {
                start: p(0, 0),
                end: nominal,
            },
        ];
        let out = assemble::<Bignum>(bag, &Q::new(1, 100)).expect("closes");
        assert_eq!(out.loops.len(), 1);
        assert_eq!(out.loops[0].len(), 3);
        for e in &out.loops[0] {
            if let Element::Arc(a) = e {
                assert!(a.is_consistent(), "assembly moved an arc off its circle");
                assert_eq!(a.delta, delta, "assembly must not inflate δ");
            }
        }
        // The gap is the file's — tiny here, and strictly separate from δ.
        assert!(out.closure_gap.sign() > 0);
        for w in out.loops[0].windows(2) {
            assert_eq!(
                w[0].end(),
                w[1].start(),
                "junctions are exact after welding"
            );
        }
    }

    /// Bulge arcs share their vertices exactly, so a bulge polyline has no junction problem at all
    /// — which is why the arc–arc refusal below is a narrow exclusion rather than a wall.
    #[test]
    fn chained_bulge_arcs_need_no_welding() {
        // Two half-turns of a unit circle: (1,0) → (−1,0) → (1,0), b = 1 each.
        let a = from_bulge::<Bignum>(p(1, 0), p(-1, 0), &Q::from_i128(1)).expect("exact");
        let b = from_bulge::<Bignum>(p(-1, 0), p(1, 0), &Q::from_i128(1)).expect("exact");
        let out = assemble::<Bignum>(vec![Element::Arc(a), Element::Arc(b)], &Q::from_i128(0))
            .expect("closes with a zero weld");
        assert_eq!(out.loops.len(), 1);
        assert_eq!(out.closure_gap, Q::from_i128(0));
    }

    /// **A loop of nothing but arcs still refuses** — the one case the junction lift cannot reach.
    ///
    /// A closed chain has one junction with no follower left to re-gauge: the last element must meet
    /// the vertex the first already owns, and moving the head's start would break the head's own far
    /// end. The walk spends its free junction there by seeding at a segment, so any loop with a
    /// segment in it is fine — and a loop of nothing but arcs has to be lucky.
    ///
    /// **Concentric arcs are lucky.** Re-gauging the follower onto the leader's endpoint puts it on
    /// the leader's own circle, and its own sweep then carries it exactly back to where the leader
    /// began — so a lens of two arcs about one centre closes at gap zero, with a `δ` for the radius
    /// that moved. Arcs about *different* centres are not: the re-gauged follower comes round at its
    /// own radius about its own centre, and lands beside the anchor rather than on it. That one is
    /// refused by name, and the cascade it would need waits for a file that has one.
    #[test]
    fn an_all_arc_loop_closes_only_when_the_re_gauge_lands_on_the_anchor() {
        let tol = ArcTolerance::report_only();
        // Both junction angles are off the quarter-turn grid, so both endpoints are *snapped* and
        // the two arcs sit on different circles. (A 0°/90° endpoint is exact and would weld
        // cleanly — which is why this test names 30°/150° explicitly.)
        let upper = |cy: Q, r: Q| {
            from_centre_angles::<Bignum>(
                [Q::from_i128(0), cy],
                &r,
                &Q::from_i128(30),
                &Q::from_i128(150),
                &tol,
            )
            .expect("certified")
        };
        let lower = |cy: Q, r: Q| {
            from_centre_angles::<Bignum>(
                [Q::from_i128(0), cy],
                &r,
                &Q::from_i128(150),
                &Q::from_i128(30),
                &tol,
            )
            .expect("certified")
        };

        // Concentric: the follower joins the leader's circle and the sweep closes it exactly.
        let a = upper(Q::from_i128(0), Q::from_i128(1));
        let b = lower(Q::from_i128(0), Q::new(1_000_001, 1_000_000));
        let (da, db) = (a.delta.clone(), b.delta.clone());
        assert!(da.sign() > 0 && db.sign() > 0);
        let out = assemble::<Bignum>(vec![Element::Arc(a), Element::Arc(b)], &Q::new(1, 10))
            .expect("the re-gauge closes a concentric lens");
        assert_eq!(out.loops.len(), 1);
        assert!(out.delta > db, "the radius that moved is charged");

        // Off-centre by a thousandth: the follower comes round about its *own* centre and misses.
        let a = upper(Q::from_i128(0), Q::from_i128(1));
        let b = lower(Q::new(-1, 1000), Q::from_i128(1));
        let out = assemble::<Bignum>(vec![Element::Arc(a), Element::Arc(b)], &Q::new(1, 10));
        assert!(
            matches!(out, Err(ImportFault::ArcJunctionGap { .. })),
            "expected a named refusal, got {out:?}"
        );
    }

    /// **The junction lift, in the shape a real drawing has it**: two arcs meeting, with a segment
    /// somewhere in the loop to absorb the closure.
    ///
    /// The follower's radius is what moves — its centre and its sweep are untouched — so the two
    /// arcs share a vertex exactly, both stay exactly on their own circles, and the move is charged
    /// to the follower's `δ` rather than being absorbed silently. The gap the file had is still
    /// reported separately, which is the distinction the module exists to keep.
    #[test]
    fn two_arcs_meeting_are_re_gauged_and_the_move_is_charged() {
        let tol = ArcTolerance::report_only();
        let one = from_centre_angles::<Bignum>(
            p(0, 0),
            &Q::from_i128(1),
            &Q::from_i128(150),
            &Q::from_i128(30),
            &tol,
        )
        .expect("certified");
        let two = from_centre_angles::<Bignum>(
            p(0, 0),
            &Q::new(1_000_001, 1_000_000),
            &Q::from_i128(30),
            &Q::from_i128(-30),
            &tol,
        )
        .expect("certified");
        let before = two.delta.clone();
        // A segment closes the figure, so the walk seeds there and both arc junctions are interior.
        let tail = two.end.clone();
        let head = one.start.clone();
        let out = assemble::<Bignum>(
            vec![Element::Arc(one), Element::Arc(two), seg(tail, head)],
            &Q::new(1, 10),
        )
        .expect("the junction lift closes it");
        assert_eq!(out.loops.len(), 1);
        assert_eq!(out.loops[0].len(), 3);
        for w in out.loops[0].windows(2) {
            assert_eq!(w[0].end(), w[1].start(), "exactly head to tail");
        }
        for e in &out.loops[0] {
            if let Element::Arc(a) = e {
                assert!(a.is_consistent(), "a re-gauged arc is still on its circle");
            }
        }
        assert!(
            out.delta > before,
            "the re-gauge is charged: {} against the entity's own {}",
            crate::num::to_decimal(&out.delta, 12),
            crate::num::to_decimal(&before, 12)
        );
        assert!(
            out.closure_gap.sign() > 0,
            "and the file's gap is still reported"
        );
    }

    /// A chain that does not close is a refusal naming the shortfall, so a reader can tell a data
    /// problem from an unsupported-entity problem.
    #[test]
    fn an_open_chain_refuses_with_the_gap() {
        let bag = vec![seg(p(0, 0), p(1, 0)), seg(p(1, 0), p(1, 1))];
        let out = assemble::<Bignum>(bag, &Q::new(1, 1000));
        match out {
            Err(ImportFault::OpenLoop { gap, .. }) => {
                assert!(gap.starts_with('1'), "the gap is the unit diagonal: {gap}")
            }
            other => panic!("expected OpenLoop, got {other:?}"),
        }
    }

    /// Loops become arrangement edges with arcs kept as arcs — a radiused outline that arrived as
    /// arcs must not leave here as chords.
    #[test]
    fn arcs_survive_into_the_profile() {
        let arc = from_bulge::<Bignum>(p(1, 0), p(-1, 0), &Q::from_i128(1)).expect("exact");
        let loops = vec![vec![Element::Arc(arc), seg(p(-1, 0), p(1, 0))]];
        let prof = to_profile(&loops);
        // The upper half-circle's x-extrema *are* its endpoints, so it is already monotone and
        // decomposes to one arc; the segment is the other edge. Neither is a chord.
        assert_eq!(prof.edges().len(), 2);
        let arcs = prof
            .edges()
            .iter()
            .filter(|e| matches!(e, geom::content::Edge::Arc(_)))
            .count();
        assert_eq!(arcs, 1, "the arc stayed an arc");
    }
}
