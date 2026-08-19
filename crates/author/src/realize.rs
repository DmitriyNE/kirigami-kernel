//! `realize` — certify the resolved structure into geometry.
//!
//! The resolver ([`crate::resolve`]) says *what* bounds the part where; this module makes it
//! certified geometry, twice over the same resolved structure:
//!
//! - [`flat_pattern`]: every boundary run's rail is fit + certified against its cutter
//!   ([`certified_rail_surface`], per region — supports differ across regions), run corners are
//!   refined to where the *fitted* rails meet (exact bisection — clean joins, the micro-cap
//!   collapses to the bisection residual), the loop is assembled as the one general
//!   [`BoundaryArc`] chain and unrolled through the connected piecewise development (chord
//!   certificates across region joins ride the anchor frames), interior holes get their own
//!   certified loops ([`surface_hole_loop`]), and the exact 2-D boolean stitches the panel —
//!   whose topology must **reproduce the resolved structure**, else the whole evaluation is
//!   refused ([`PartFault::TopologyMismatch`]): a mis-resolution cannot ship.
//! - [`solid_brep`]: the same boundary re-certified at the low-degree STEP profile
//!   ([`RailFit::occt_low`], corners dyadic-snapped — OCCT's `f64` edge tolerance needs both),
//!   emitted as the piecewise inner/outer chains + hole rails `brep_trim_solid_regions` sews
//!   into a certified watertight solid.

use crate::part::OpKind;
use crate::part::{
    BuiltRegions, Cutter, FlatPattern, OpReport, Part, PartFault, RegionEcho, ResolveReport,
};
use crate::resolve::{BranchSide, Label, Structure, wall_of};
use certify_core::Verdict;
use develop::cut::{CutSurface, cut_mu_form};
use develop::part::Development;
use develop::unroll::{BoundaryArc, FlatOutline, UnrollFault, unroll_trim_loop};
use export::brep::Brep;
use export::brep_build::{HoleRail, WirePoint, brep_trim_solid_regions};
use export::cut_oracle::RootPick;
use export::trim::{
    HoleLoop, RailFit, assemble_flat, bisect_root, certified_rail_surface, chord_pcurve,
    flat_to_poly, hole_poly, hole_rail, shadow_hole_loops, surface_hole_loop,
};
use lattice::{Backend, Interval, Rat, RatFunc};

/// Does the wall's µ̂-quadratic open **upward** over `span` — i.e. is "inside the cutter" the
/// interval *between* its roots?
///
/// Sampled at the span's midpoint, because it only chooses which branch the oracle proposes;
/// `cut_fit` certifies whatever comes back, so a wrong answer costs a refusal and never a wrong
/// `Verified`. A vanishing or absent leading coefficient means one root, where the choice is moot
/// — `true` keeps the historical mapping.
fn mu_form_opens_up<B: Backend>(
    chart: &geom::chart::Chart<B>,
    wall: &CutSurface<B>,
    span: &Interval<B>,
) -> bool {
    let mid = span.lo.add(&span.hi).div(&Rat::from_i128(2));
    match cut_mu_form(chart, wall, &Rat::from_i128(0)).and_then(|f| f.a.eval(&mid)) {
        Some(a) => a.sign() >= 0,
        None => true,
    }
}

/// One wall's real σ-window within a region: the stretch between consecutive tangent rulings on
/// which its µ̂-quadratic has roots at all, and **which ends are tangent rulings**.
///
/// The distinction is not bookkeeping. A fit stands off from a window end because the branch has a
/// √-tail with unbounded slope there — which is true of a tangent ruling and false of the band's
/// own edge. Insetting at a band edge buys nothing and costs reach: it shortens the rail by the
/// inset at exactly the σ where a derived σ-end puts the boundary, and the segment then has no
/// certified rail under it.
struct Window<B: lattice::Backend> {
    span: Interval<B>,
    lo_is_tangent: bool,
    hi_is_tangent: bool,
}

/// One piecewise boundary chain: ordered contiguous `(σ-band, rail)` pieces (the
/// `brep_trim_solid_regions` currency).
type Chain<B> = Vec<(Interval<B>, RatFunc<B>)>;

/// The solid evaluator's payload: the exact B-rep, the max rail ε, and the report echo.
type SolidParts<B> = (Brep<B>, Rat<B>, ResolveReport<B>);

/// A realization refusal: a typed fault, or a loose (refinable) certified bound.
pub(crate) enum RErr<B: Backend> {
    Fault(PartFault),
    Loose(Rat<B>),
}

/// Translate a realization refusal into the evaluator's verdict.
macro_rules! bail {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(RErr::Fault(f)) => return Verdict::Refuted(f),
            Err(RErr::Loose(e)) => {
                return Verdict::Unresolved(e);
            }
        }
    };
}

/// One fitted + certified rail piece: a label realized on one region, over the σ-span the
/// certificate actually covers.
struct RailPiece<B: Backend> {
    label: Label,
    region: usize,
    mu: RatFunc<B>,
    /// The span `certified_rail_surface` was given — **not** the span the caller asked for, since
    /// the fit clamps to the wall's disc-positive window. Checked against the chain segments before
    /// any of this is used ([`certify_boundary`], step 3).
    span: Interval<B>,
}

/// A **mid-chain splice**: the certified path across a flank crossing — §12.4's p-curve end, met
/// in the middle of a chain rather than at a σ-end.
///
/// Two quadric walls whose profile edges are both tangent to the same straight edge (the device
/// drawing's fillet–flank–fillet corner) have µ̂-windows that **abut at a shared tangent ruling**:
/// the flank is radial, so the ruling through it is tangent to both fillets at once. The two
/// graph rails never cross — they are joined by a stretch of boundary *along* that ruling — and
/// neither can be certified up to it (the branch turns vertical there). So each chain segment
/// stops at its own certificate's edge, and the splice owns the gap `[edge_a, edge_b]`.
struct Splice<B: Backend> {
    /// Where the left rail's certified span ends — its segment's new `to`.
    edge_a: Rat<B>,
    /// Where the right rail's certified span begins — its segment's new `from`.
    edge_b: Rat<B>,
    /// The left rail's fitted value at `edge_a` (one end of the solid's chord).
    v_a: Rat<B>,
    /// The right rail's fitted value at `edge_b` (the other).
    v_b: Rat<B>,
    /// The traced route across the gap, σ-ascending: the left wall's tail into its tangent
    /// vertex, the connector along the flank, the right wall's tail out. The flat pattern draws it
    /// piece-by-piece; the solid chords it at the export sagitta budget ([`splice_chain`]) — it
    /// used to take the single chord `(edge_a, v_a) → (edge_b, v_b)` instead, which was #305 once
    /// splices grew from µm-fine tails into whole caps and domes. Empty only when built with
    /// `traced_splices: false`, which then degenerates the solid's stretch to that single chord.
    curves: Vec<develop::pcurve::PCurve<B>>,
    /// The certified bound over whichever route was built (folded into the boundary's ε).
    eps: Rat<B>,
}

/// One refined run corner: the σ where the two adjacent rails meet, or the gap a [`Splice`] owns.
enum Corner<B: Backend> {
    At(Rat<B>),
    Gap(Rat<B>, Rat<B>),
}

/// The certified boundary: the per-region rail pieces, the per-side chain segments
/// `(from, to, label)` covering the domain, and the max rail ε.
struct Boundary<B: Backend> {
    pieces: Vec<RailPiece<B>>,
    upper_segs: Vec<(Rat<B>, Rat<B>, Label)>,
    lower_segs: Vec<(Rat<B>, Rat<B>, Label)>,
    /// The mid-chain splices per side, in ascending σ; each sits between the segment ending at its
    /// `edge_a` and the one beginning at its `edge_b`.
    upper_splices: Vec<Splice<B>>,
    lower_splices: Vec<Splice<B>>,
    /// The turn arc closing each σ-end (`[lower end, upper end]`), where that end is a **smooth
    /// pinch** — a tangent ruling of one quadric wall, which no graph rail reaches. Where it is
    /// `None` the end closes with a ruling cap, as it always did. When an arc is present the
    /// outermost segment on *both* chains has been removed: the arc replaces
    /// `[upper rail tail] + [cap] + [lower rail head]`, joining the graph rails at the two
    /// junctions the run-corner refinement located.
    end_arcs: [Option<Vec<develop::pcurve::PCurve<B>>>; 2],
    eps: Rat<B>,
}

/// The straight domain segment between two `(σ, µ̂)` points, as a p-curve over `t ∈ [0, 1]`.
fn domain_segment<B: Backend>(a: &[Rat<B>; 2], b: &[Rat<B>; 2]) -> develop::pcurve::PCurve<B> {
    let lin = |p: &Rat<B>, q: &Rat<B>| {
        RatFunc::from_poly(lattice::Poly::from_coeffs(vec![p.clone(), q.sub(p)]))
    };
    develop::pcurve::PCurve {
        sigma: lin(&a[0], &b[0]),
        mu: lin(&a[1], &b[1]),
        domain: Interval {
            lo: Rat::from_i128(0),
            hi: Rat::from_i128(1),
        },
    }
}

/// Per region, per op, per wall: the wall's µ̂-discriminant together with the brackets isolating
/// its tangent rulings, or `None` where the wall is affine and has no windows.
///
/// The discriminant rides along because the brackets alone do not say **which** of the stretches
/// they cut the band into the wall is real on. A gap between two consecutive tangent rulings is
/// disc-positive or disc-negative depending on the wall, and a rail clamped into the negative one
/// is a rail over σ where the cut does not exist.
type DiscRoots<B> = Vec<Vec<Vec<Option<(RatFunc<B>, Vec<Interval<B>>)>>>>;

/// Detect and build the [`Splice`] at one run corner, or `None` where the corner is not a flank
/// crossing (the ordinary case — the caller then refines it as a rail crossing, as ever).
///
/// The detection is exact, not heuristic, and every clause names a property the construction
/// needs:
/// - **same op, distinct walls**, of which at least one **turns**: it has an isolated
///   discriminant-root bracket inside this corner's gap, where its window ends and its rail's
///   certificate stops. When both turn (a fillet–flank–fillet corner, #294) the two brackets must
///   *overlap* — the windows abut at one shared tangent ruling. A side with no root in the gap
///   **continues** (#296's mixed corner): its wall crosses the flank transversally, its rail is
///   certified straight across the gap, and its own certified edge is where the connector starts.
///   The lug's rim against its tangent nose arc is the geometry that forces it.
/// - **a middle wall between them in the profile cycle, affine in µ̂** — the flank itself. It is
///   what the connector is certified against: the true boundary between the handoff points runs
///   *along* that wall. (A flank is a plane through the chart's apex, so its pullback crosses
///   µ̂ = 0 identically except at its own azimuth, where the ruling lies *in* the plane — which is
///   where a wall tangent to it turns. The tangency root and the flank's own ruling are the same
///   σ, which is why one isolation serves both claims.)
///
/// The traced route (`traced`): each **turning** wall's own certified tail from the rail's
/// certified edge into the tangent vertex ([`develop::cut::quadric_tail`] — PC.3's construction,
/// graded for the one-tangency stretch); the connector joins the two handoff points — a tangent
/// vertex, or a continuing rail's certified endpoint — and is certified by
/// [`develop::cut::pcurve_cut_fit`]. The chord route (solid): one straight piece between the
/// rails' own endpoint values, certified the same way — it deviates from the true corner by the
/// √-tails the rails could not reach, and the certificate against the flank wall is exactly a
/// bound on that.
///
/// A tail or certificate that comes back `Unresolved` propagates as a loose (refinable) bound; a
/// structural disagreement (`Refuted`) falls back to `None`, so the refusal the caller then
/// reports is the honest pre-existing one.
#[allow(clippy::too_many_arguments)]
fn flank_splice<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    bands: &[Interval<B>],
    disc_roots: &DiscRoots<B>,
    pieces: &[RailPiece<B>],
    l: Label,
    r: Label,
    gap_lo: &Rat<B>,
    gap_hi: &Rat<B>,
    traced: bool,
) -> Result<Option<Splice<B>>, RErr<B>> {
    use core::cmp::Ordering;
    let op = l.0;
    if r.0 != op {
        return Ok(None);
    }
    let (wl, wr) = (wall_of(l), wall_of(r));
    if wl == wr {
        return Ok(None);
    }
    // One region only: the splice's curves live on one chart. A gap straddling a region join
    // stays a refusal until a real part needs it.
    let Some(ri) = bands.iter().position(|band| {
        band.lo.cmp(gap_lo) != Ordering::Greater && gap_hi.cmp(&band.hi) != Ordering::Greater
    }) else {
        return Ok(None);
    };
    // Which sides **turn**: an isolated root of the wall's discriminant inside this gap — the same
    // isolation its window was clamped to, so "the window ends here" and "the boundary hands off
    // here" are claims about the same root. `l`'s window ends at its root and `r`'s begins at its,
    // so a turning `l` takes the stretch before its root and a turning `r` the stretch after. A
    // wall with no root in the gap **continues** across it and needs no window at all.
    //
    // When the root is the wall's *first* (or *last*) tangent ruling in this band, the stretch is
    // bounded by the band itself — the same outermost window `window_for` names. Demanding a
    // neighbouring bracket instead refused exactly the walls that are real from the band's edge up
    // to their one tangent ruling, which is where a derived σ-end puts the bore's rail (#302).
    let in_gap = |iv: &Interval<B>| {
        gap_lo.cmp(&iv.lo) != Ordering::Greater && iv.hi.cmp(gap_hi) != Ordering::Greater
    };
    type Turn<B> = Option<(Interval<B>, Interval<B>)>;
    let turn_of = |w: usize, needs_preceding: bool| -> Turn<B> {
        let (_, b) = disc_roots[ri][op].get(w).and_then(|x| x.as_ref())?;
        let k = b.iter().position(&in_gap)?;
        let win = if needs_preceding {
            Interval {
                lo: if k == 0 {
                    bands[ri].lo.clone()
                } else {
                    b[k - 1].hi.clone()
                },
                hi: b[k].lo.clone(),
            }
        } else {
            Interval {
                lo: b[k].hi.clone(),
                hi: if k + 1 >= b.len() {
                    bands[ri].hi.clone()
                } else {
                    b[k + 1].lo.clone()
                },
            }
        };
        Some((b[k].clone(), win))
    };
    // Neither turning is no flank crossing at all: two rails that really cross, refined as ever.
    let (turn_l, turn_r) = (turn_of(wl, true), turn_of(wr, false));
    if turn_l.is_none() && turn_r.is_none() {
        return Ok(None);
    }
    // Both turning: the two windows must abut at ONE shared tangent ruling — overlapping brackets.
    if let (Some((rl, _)), Some((rr, _))) = (&turn_l, &turn_r)
        && (rl.lo.cmp(&rr.hi) == Ordering::Greater || rr.lo.cmp(&rl.hi) == Ordering::Greater)
    {
        return Ok(None);
    }
    // The connector's own wall: the profile edge between the two, affine in µ̂.
    let walls = part.ops[op]
        .1
        .walls()
        .map_err(|_| RErr::Fault(PartFault::CutUnresolved { op }))?;
    let nw = walls.len();
    if nw < 3 {
        return Ok(None);
    }
    let (m1, m2) = ((wl + 1) % nw, (wl + nw - 1) % nw);
    let m = if (m1 + 1) % nw == wr {
        m1
    } else if (m2 + nw - 1) % nw == wr {
        m2
    } else {
        return Ok(None);
    };
    let chart = &built.charts[ri];
    let zero = Rat::from_i128(0);
    match cut_mu_form(chart, &walls[m], &zero) {
        Some(f) if f.a.is_zero() => {}
        _ => return Ok(None),
    }
    // The rails' certified edges, and their fitted values there.
    let (Some(pa), Some(pb)) = (
        find_piece(pieces, l, ri, gap_lo),
        find_piece(pieces, r, ri, gap_hi),
    ) else {
        return Ok(None);
    }; // The pieces must actually be certified AT this corner. `find_piece`'s trailing arms return a
    // same-label piece from anywhere as a last resort, and on a wrapping chart the same wall bounds
    // on a second pass half a turn away (#293) — so when this corner's own piece is missing (its
    // fit was recorded rather than raised, step 2), the fallback is the *other pass's* rail, whose
    // span and fitted values are about a different stretch of boundary entirely. Every use below —
    // the handoff edges, `mu.eval` at them, the tails — would be arithmetic on that wrong rail, so
    // the corner is declined instead and the coverage check reports the fit's own recorded reason.
    //
    // What "at this corner" means is per side. A **continuing** side's rail is certified straight
    // across the gap, so its span covers the gap edge. A **turning** side's span is clamped into
    // its own window less the fit inset, and the gap edge — the neighbouring run's outermost
    // sample — can land inside that inset sliver; the honest claim there is that the span lies in
    // the window this turn opens or closes, which the wrong-pass fallback never does (its span is
    // in the same wall's *other* window, half a turn away).
    let holds = |p: &RailPiece<B>, s: &Rat<B>| {
        p.span.lo.cmp(s) != Ordering::Greater && s.cmp(&p.span.hi) != Ordering::Greater
    };
    let overlaps = |p: &RailPiece<B>, w: &Interval<B>| {
        p.span.lo.cmp(&w.hi) != Ordering::Greater && w.lo.cmp(&p.span.hi) != Ordering::Greater
    };
    let ok_a = match &turn_l {
        None => holds(pa, gap_lo),
        Some((_, win)) => overlaps(pa, win),
    };
    let ok_b = match &turn_r {
        None => holds(pb, gap_hi),
        Some((_, win)) => overlaps(pb, win),
    };
    if !ok_a || !ok_b {
        return Ok(None);
    }
    // A turning side hands off at its rail's certified edge — the window inset just before its
    // tangency. A continuing side's rail is certified straight ACROSS the gap and past the handoff
    // (its span is hulled out to the neighbouring run's first sample, a grid point on the wrong
    // side of the flank), so its span end says nothing about where the boundary turns: the handoff
    // is the *turning* wall's own root, taken at its bracket edge and clamped into the continuing
    // rail's certificate.
    //
    // (Handing off *earlier* — walking the traced tail deeper into the window so the rail never
    // chordizes the curl — was tried and honestly refused: the tail's pieces are certified one by
    // one at a fixed subdivision, and stretched over a wide window they read looser than the
    // clearance. The curl's chordization is an *emission* problem and is solved there: the chain
    // assembly √-grades the rail's own chords into a spliced end.)
    let edge_a = match (&turn_l, &turn_r) {
        (None, Some((root_r, _))) => rmax(&pa.span.lo, &rmin(&pa.span.hi, &root_r.lo)),
        _ => pa.span.hi.clone(),
    };
    let edge_b = match (&turn_r, &turn_l) {
        (None, Some((root_l, _))) => rmin(&pb.span.hi, &rmax(&pb.span.lo, &root_l.hi)),
        _ => pb.span.lo.clone(),
    };
    if edge_a.cmp(&edge_b) != Ordering::Less {
        return Ok(None);
    }
    let (Some(v_a), Some(v_b)) = (pa.mu.eval(&edge_a), pb.mu.eval(&edge_b)) else {
        return Err(RErr::Fault(PartFault::Pole));
    };

    let mut eps = Rat::from_i128(0);
    let certify = |curve: &develop::pcurve::PCurve<B>,
                   wall: &CutSurface<B>,
                   eps: &mut Rat<B>|
     -> Result<bool, RErr<B>> {
        match develop::cut::pcurve_cut_fit(
            chart,
            curve,
            wall,
            &zero,
            32,
            &part.clearance,
            &part.cfg,
        ) {
            Verdict::Verified(v) => {
                *eps = rmax(eps, &v.eps);
                Ok(true)
            }
            Verdict::Unresolved(e) => Err(RErr::Loose(e)),
            Verdict::Refuted(_) => Ok(false),
        }
    };
    let curves = if traced {
        // A turning side contributes its wall's own traced tail — from the rail's certified edge
        // into the tangent vertex. A continuing side has no vertex to walk into and contributes
        // nothing: its rail's certified endpoint IS where the connector starts.
        let make_tail = |turn: &Turn<B>,
                         wi: usize,
                         edge: &Rat<B>,
                         val: &Rat<B>,
                         vertex_is_max: bool,
                         eps: &mut Rat<B>|
         -> Result<Option<Vec<develop::pcurve::PCurve<B>>>, RErr<B>> {
            let Some((_, win)) = turn else {
                return Ok(Some(Vec::new()));
            };
            // The dedicated tail tracer: the branch from the rail's certified edge into the
            // tangent vertex, √-graded toward the vertex so the chords are equal-turn — the
            // resolution lands where the turning is, at every `segments`. (Its two predecessors
            // are recorded on [`develop::cut::quadric_tail`]: tracing the wall's *full* window
            // gave the tail `n·√f` of the loop's `n` pieces — two or three chords carrying ~50°
            // — and tracing a padded sub-window inherited the loop's both-end grading, so the
            // junction chord stayed coarse however fine the budget.)
            match develop::cut::quadric_tail(
                chart,
                &walls[wi],
                win,
                edge,
                val,
                vertex_is_max,
                (part.segments / 2).max(8),
                &zero,
                &part.clearance,
                &part.cfg,
            ) {
                Verdict::Verified(t) => {
                    *eps = rmax(eps, &t.eps);
                    Ok(Some(t.pieces))
                }
                Verdict::Unresolved(e) => Err(RErr::Loose(e)),
                Verdict::Refuted(_) => Ok(None),
            }
        };
        let Some(tail_a) = make_tail(&turn_l, wl, &edge_a, &v_a, true, &mut eps)? else {
            return Ok(None);
        };
        let Some(tail_b) = make_tail(&turn_r, wr, &edge_b, &v_b, false, &mut eps)? else {
            return Ok(None);
        };
        // The connector's endpoints: a turning side's tangent vertex, a continuing side's own
        // certified rail edge.
        let va = match tail_a.last() {
            Some(last) => last
                .eval(&last.domain.hi)
                .ok_or(RErr::Fault(PartFault::Pole))?,
            None => [edge_a.clone(), v_a.clone()],
        };
        let vb = match tail_b.first() {
            Some(first) => first
                .eval(&first.domain.lo)
                .ok_or(RErr::Fault(PartFault::Pole))?,
            None => [edge_b.clone(), v_b.clone()],
        };
        // The connector, as an exactly-vertical piece plus an exactly-horizontal one rather than
        // the diagonal between the vertices. The two tangent vertices sit on the same ruling to
        // within their brackets, so the diagonal is ~10⁻⁶ off vertical — but the unroll can carry
        // a *vertical* piece exactly (a ruling segment develops to a straight edge, a `Cap`),
        // while a merely near-vertical one goes through the generic chord bound, whose enclosure
        // over a millimetres-long µ̂ span reads several mm however true the piece is. Both pieces
        // still certify against the flank wall; the assembly emits them as the exact arc kinds.
        let elbow = [va[0].clone(), vb[1].clone()];
        let mut cs = tail_a;
        for piece in [domain_segment(&va, &elbow), domain_segment(&elbow, &vb)] {
            let a = piece
                .eval(&piece.domain.lo)
                .ok_or(RErr::Fault(PartFault::Pole))?;
            let bpt = piece
                .eval(&piece.domain.hi)
                .ok_or(RErr::Fault(PartFault::Pole))?;
            if a[0].cmp(&bpt[0]) == Ordering::Equal && a[1].cmp(&bpt[1]) == Ordering::Equal {
                continue;
            }
            if !certify(&piece, &walls[m], &mut eps)? {
                return Ok(None);
            }
            cs.push(piece);
        }
        cs.extend(tail_b);
        cs
    } else {
        let chord = domain_segment(
            &[edge_a.clone(), v_a.clone()],
            &[edge_b.clone(), v_b.clone()],
        );
        if !certify(&chord, &walls[m], &mut eps)? {
            return Ok(None);
        }
        Vec::new()
    };
    Ok(Some(Splice {
        edge_a,
        edge_b,
        v_a,
        v_b,
        curves,
        eps,
    }))
}

/// The smaller/larger of two rationals.
fn rmin<B: Backend>(a: &Rat<B>, b: &Rat<B>) -> Rat<B> {
    if a.cmp(b) == core::cmp::Ordering::Less {
        a.clone()
    } else {
        b.clone()
    }
}
fn rmax<B: Backend>(a: &Rat<B>, b: &Rat<B>) -> Rat<B> {
    if a.cmp(b) == core::cmp::Ordering::Less {
        b.clone()
    } else {
        a.clone()
    }
}

/// Append a [`Splice`]'s stretch of a solid chain: the **traced route** across the gap — the same
/// tails-and-connector the flat pattern draws — chorded at the export sagitta budget
/// ([`chord_pcurve`]) into contiguous affine `(band, rail)` pieces, the only currency the solid
/// builder takes.
///
/// This replaces the single certified chord per splice, which was #305: splices had grown from
/// µm-fine tails into whole caps and domes while the solid still hulled each one, so the device's
/// lug came out with sides 15° off their rulings and a flat top at the two rails' shared level
/// where the drawing has a dome. The µm-fine-tail concern that motivated the chord is handled
/// where it belongs instead: interior route points snap to the STEP dyadic grid, and a piece
/// narrower than the export profile's step (`2⁻²⁰`, `export::trim`'s `MIN_STEP`) folds into its
/// neighbour, so nothing below OCCT's vertex tolerance is emitted.
///
/// The chain is a µ̂(σ) graph, so an exactly-vertical stretch (the connector's elbow — a drawn
/// radial flank) cannot be a piece of it: the walk keeps only σ-advancing points, and a vertical
/// drop is absorbed into the next advancing piece's slope — the hull of the *unrepresentable
/// stretch alone*, no longer of the whole splice. Both ends stay pinned to the rails' own
/// `(edge, value)` corners, so the chain stays contiguous with its rail pieces exactly as before.
/// A splice carrying no route (`curves` empty) degenerates to the old single chord by
/// construction — the walk emits just the pinned end-to-end piece.
fn splice_chain<B: Backend>(sp: &Splice<B>, out: &mut Chain<B>) -> Result<(), PartFault> {
    use core::cmp::Ordering;
    let affine = |a: &(Rat<B>, Rat<B>), b: &(Rat<B>, Rat<B>)| {
        let slope = b.1.sub(&a.1).div(&b.0.sub(&a.0));
        (
            Interval {
                lo: a.0.clone(),
                hi: b.0.clone(),
            },
            RatFunc::from_poly(lattice::Poly::from_coeffs(vec![
                a.1.sub(&slope.mul(&a.0)),
                slope,
            ])),
        )
    };
    let mut pts: Vec<(Rat<B>, Rat<B>)> = Vec::new();
    for curve in &sp.curves {
        if chord_pcurve(curve, &mut pts).is_none() {
            return Err(PartFault::Pole);
        }
    }
    let min_w = Rat::<B>::new(1, 1i128 << 20);
    let mut cur = (sp.edge_a.clone(), sp.v_a.clone());
    for (s, m) in pts {
        let s = snap30(&s);
        // Interior, σ-advancing, and at least an export step wide on both sides.
        if s.sub(&cur.0).cmp(&min_w) != Ordering::Greater {
            continue;
        }
        if sp.edge_b.sub(&s).cmp(&min_w) != Ordering::Greater {
            break;
        }
        let p = (s, m);
        out.push(affine(&cur, &p));
        cur = p;
    }
    let end = (sp.edge_b.clone(), sp.v_b.clone());
    if end.0.cmp(&cur.0) == Ordering::Greater {
        out.push(affine(&cur, &end));
    }
    Ok(())
}

/// The pieces of `[a, b]` split at region joins: `(from, to, region index)` in ascending σ.
fn span_pieces<B: Backend>(
    regions: &[Interval<B>],
    a: &Rat<B>,
    b: &Rat<B>,
) -> Vec<(Rat<B>, Rat<B>, usize)> {
    use core::cmp::Ordering;
    let mut out = Vec::new();
    for (ri, band) in regions.iter().enumerate() {
        let lo = rmax(a, &band.lo);
        let hi = rmin(b, &band.hi);
        if lo.cmp(&hi) == Ordering::Less {
            out.push((lo, hi, ri));
        }
    }
    out.sort_by(|x, y| x.0.cmp(&y.0));
    out
}

/// Find `label`'s rail piece covering `sigma`, preferring `region`.
///
/// One label can have **several** pieces in one region: a wall that bounds in two separated σ-runs
/// is fitted once per run, and on a wrapping chart that is the normal case rather than the odd one
/// (#293). So σ identifies the piece and region only breaks ties — matching on region alone returns
/// whichever piece came first, which is a rail from the wrong side of the chart. The region-only
/// arms stay as the fallback for a σ inside no piece's span, which is the shape every caller had
/// while there was only ever one.
fn find_piece<'a, B: Backend>(
    pieces: &'a [RailPiece<B>],
    label: Label,
    region: usize,
    sigma: &Rat<B>,
) -> Option<&'a RailPiece<B>> {
    use core::cmp::Ordering;
    let covers = |p: &RailPiece<B>| {
        p.span.lo.cmp(sigma) != Ordering::Greater && sigma.cmp(&p.span.hi) != Ordering::Greater
    };
    pieces
        .iter()
        .find(|p| p.label == label && p.region == region && covers(p))
        .or_else(|| pieces.iter().find(|p| p.label == label && covers(p)))
        .or_else(|| {
            pieces
                .iter()
                .find(|p| p.label == label && p.region == region)
        })
        .or_else(|| pieces.iter().find(|p| p.label == label))
}

/// Evaluate the rail for `label` at σ, preferring the piece whose region contains σ.
fn rail_at<B: Backend>(
    pieces: &[RailPiece<B>],
    regions: &[Interval<B>],
    label: Label,
    sigma: &Rat<B>,
) -> Option<Rat<B>> {
    use core::cmp::Ordering;
    let ri = regions
        .iter()
        .position(|band| {
            band.lo.cmp(sigma) != Ordering::Greater && sigma.cmp(&band.hi) != Ordering::Greater
        })
        .unwrap_or(0);
    find_piece(pieces, label, ri, sigma)?.mu.eval(sigma)
}

/// Snap a σ to the `2⁻³⁰` dyadic grid (the STEP corner discipline — huge-denominator corner σ
/// make exported Bézier control points drift off OCCT's `f64` vertices).
fn snap30<B: Backend>(x: &Rat<B>) -> Rat<B> {
    export::approx::f64_to_rat::<B>(export::approx::rat_to_f64(x), 30)
}

/// Steps 1–3 of both evaluators: fit + certify every boundary label's rail per region (spans
/// clamped to the cutter's exact two-tangent extent), refine the run corners on the fitted
/// rails, and fold the runs into per-side chain segments covering the domain. `snap_corners`
/// applies the STEP dyadic snap to the refined junctions; `traced_splices` picks each flank
/// crossing's route — the traced tails-and-connector for the flat pattern, the single certified
/// chord for the solid (see [`Splice`]).
fn certify_boundary<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: &Structure<B>,
    fit_base: RailFit,
    snap_corners: bool,
    traced_splices: bool,
) -> Result<Boundary<B>, RErr<B>> {
    use core::cmp::Ordering;
    let bands: Vec<Interval<B>> = part.regions.iter().map(|r| r.band.clone()).collect();
    // The **derived** σ-extent, not the authored band (AUTH.3b). They coincide unless a cutter
    // terminates the blank; where they differ, the boundary closes at the derived ends and
    // realizing over the band would draw a loop through σ the ops left empty.
    let domain = structure.domain.clone();
    let domain_width = domain.hi.sub(&domain.lo);
    let runs = &structure.runs;
    if runs.is_empty() {
        return Err(RErr::Fault(PartFault::EmptyRegion));
    }
    let mut eps = Rat::from_i128(0);

    // — 1. The σ-usage hull per boundary label (runs + event brackets + domain ends). —
    let mut labels: Vec<Label> = Vec::new();
    for run in runs {
        for lab in [run.lower, run.upper] {
            if !labels.contains(&lab) {
                labels.push(lab);
            }
        }
    }
    // The σ-hulls a label is used over — one per maximal group of CONSECUTIVE runs, not one hull
    // over all of them.
    //
    // A label that bounds in two separated runs must be fitted twice, and on a wrapping chart that
    // is not an edge case: a chart covering more than a turn passes every azimuth twice, so a wall
    // bounding at one azimuth bounds at two σ. Hulling those together spans the gap between them,
    // where the wall is not merely unfitted but *absent* — the device drawing's R 0.25 root fillet
    // subtends 7.6° of azimuth and was handed a 60°+ span, on which the oracle rightly declined
    // (`disc < 0` at its first node, #293).
    //
    // Splitting is safe for the *other* reason runs go non-consecutive — a boundary spliced by a
    // turn arc, or another label bounding between — only because a fit that does not certify is
    // now recorded rather than raised: those groups are slivers beside a tangent ruling that no
    // graph rail can follow, and step 3′ deletes their segments anyway.
    //
    // Within a group the hull is unchanged: each run extends into its neighbours' brackets, because
    // the true corner lies between the samples, and to the domain ends on the outermost runs.
    let hulls_of = |label: Label| -> Vec<(Rat<B>, Rat<B>)> {
        let mut out: Vec<(Rat<B>, Rat<B>)> = Vec::new();
        let mut prev: Option<usize> = None;
        for (i, run) in runs.iter().enumerate() {
            if run.lower != label && run.upper != label {
                continue;
            }
            let a = if i == 0 {
                domain.lo.clone()
            } else {
                runs[i - 1].hi.clone()
            };
            let b = if i + 1 == runs.len() {
                domain.hi.clone()
            } else {
                runs[i + 1].lo.clone()
            };
            match prev {
                Some(p) if p + 1 == i => {
                    let last = out.last_mut().expect("a run started the group");
                    last.0 = rmin(&last.0, &a);
                    last.1 = rmax(&last.1, &b);
                }
                _ => out.push((a, b)),
            }
            prev = Some(i);
        }
        out
    };

    // — 2. Fit + certify each label's rail per region (the A4 shape). —
    // A quadratic cutter's branch is only real between two tangent rulings, and a wide gore
    // meets a cylinder along several such windows (one per ruling sheet) — fitting past a
    // window's ends makes the oracle decline. So a cylinder label's span clamps to the
    // disc-positive window that contains it (inset a hair, the hole-loop margin doctrine).
    // Planes have no windows.
    // Windowing is a property of the WALL, not of the cutter variant: a wall whose µ̂-pullback is a
    // genuine quadratic (`a ≢ 0`) is real only between tangent rulings, an affine one everywhere.
    // Reading it that way rather than matching on `Cutter` is what lets a multi-walled cutter join
    // — and it reproduces the old behaviour exactly, since a cylinder's wall is quadratic and a
    // half-space's is not.
    let mut disc_roots: DiscRoots<B> = Vec::new();
    for (ri, band) in bands.iter().enumerate() {
        let mut row = Vec::with_capacity(part.ops.len());
        for (op, (_, cutter)) in part.ops.iter().enumerate() {
            let walls = cutter
                .walls()
                .map_err(|_| RErr::Fault(PartFault::CutUnresolved { op }))?;
            let mut per_wall = Vec::with_capacity(walls.len());
            for wall in &walls {
                // The same exact isolation the resolver derived its windows from, so the span this
                // clamps to and the window that attributed the hole are the *same* interval — a
                // sampled approximation on one side and an exact one on the other would clamp a
                // fit to a window the structure was never resolved over.
                let form = develop::cut::cut_mu_form(&built.charts[ri], wall, &Rat::from_i128(0));
                per_wall.push(match form {
                    Some(f) if !f.a.is_zero() => {
                        develop::cut::tangent_events(&f, band, &crate::resolve::tangent_tol())
                            .ok()
                            .map(|roots| (f.disc(), roots))
                    }
                    _ => None,
                });
            }
            row.push(per_wall);
        }
        disc_roots.push(row);
    }
    // **The wall's real σ-window**: the maximal stretch between consecutive tangent rulings on
    // which the µ̂-quadratic actually has roots — the only σ where a branch of this wall exists to
    // be fitted at all. Chosen as the disc-positive stretch overlapping `raw` most, so the clamp
    // follows the span the boundary asks about rather than whichever side of a tangency a midpoint
    // happens to fall on.
    //
    // Two things the brackets alone do not give, and both cost the demo a rail (#302):
    //
    // - **The outermost stretches count.** Reading only the gaps *between* consecutive brackets
    //   leaves `band.lo → first root` and `last root → band.hi` unnameable, and a wall real only up
    //   to its first tangent ruling has its entire rail in exactly one of those. With no window the
    //   fit ladder loses its clamp *and* its second rung, and the oracle is handed a raw hull that
    //   runs off the end of the wall.
    // - **A gap is not a window until its sign says so.** `disc` alternates sign across simple
    //   roots, so half the gaps are stretches where the cut is *not real*; clamping into one is
    //   worse than not clamping. The sign is read at the stretch's own midpoint, which decides the
    //   whole stretch because the brackets isolate every root in the band — no root lies inside.
    let window_for = |ri: usize, op: usize, wall: usize, raw: &Interval<B>| -> Option<Window<B>> {
        let (disc, brackets) = disc_roots[ri][op].get(wall)?.as_ref()?;
        let band = &bands[ri];
        let mut stretches: Vec<Window<B>> = Vec::with_capacity(brackets.len() + 1);
        let mut lo = band.lo.clone();
        let mut lo_is_tangent = false;
        for b in brackets {
            stretches.push(Window {
                span: Interval {
                    lo,
                    hi: b.lo.clone(),
                },
                lo_is_tangent,
                hi_is_tangent: true,
            });
            lo = b.hi.clone();
            lo_is_tangent = true;
        }
        stretches.push(Window {
            span: Interval {
                lo,
                hi: band.hi.clone(),
            },
            lo_is_tangent,
            hi_is_tangent: false,
        });
        let mut best: Option<(Rat<B>, Window<B>)> = None;
        for w in stretches {
            if w.span.lo.cmp(&w.span.hi) != Ordering::Less {
                continue;
            }
            let (ov_lo, ov_hi) = (rmax(&w.span.lo, &raw.lo), rmin(&w.span.hi, &raw.hi));
            if ov_lo.cmp(&ov_hi) != Ordering::Less {
                continue;
            }
            let mid = w.span.lo.add(&w.span.hi).div(&Rat::from_i128(2));
            match disc.eval(&mid) {
                Some(d) if d.sign() > 0 => {}
                _ => continue,
            }
            let width = ov_hi.sub(&ov_lo);
            let better = match &best {
                Some((w0, _)) => w0.cmp(&width) == Ordering::Less,
                None => true,
            };
            if better {
                best = Some((width, w));
            }
        }
        best.map(|(_, w)| w)
    };
    // **Certify what the boundary uses, not what it might use.**
    //
    // The chains are not final here: step 3′ *deletes* the outermost segment of each chain — and
    // in the whole-side case the entire lower chain — replacing them with a turn arc. A rail whose
    // every segment is about to be deleted is one this function was demanding a certificate for and
    // then throwing away, and near a tangent ruling a graph rail cannot be certified at all. So a
    // fit that does not certify is **recorded, not raised**: [`covered`] raises it, with the reason
    // the fit actually gave, if a *surviving* segment turns out to need it. Nothing is weakened —
    // a rail the boundary uses must still certify, which is the whole of the obligation.
    //
    // `Ok(ε)` is a loose fit (refinable), `Err(fault)` a refusal.
    let mut deferred: Vec<(Label, Result<Rat<B>, PartFault>)> = Vec::new();
    let mut pieces: Vec<RailPiece<B>> = Vec::new();
    for &label in &labels {
        for (lo, hi) in hulls_of(label) {
            for (span_lo, span_hi, ri) in span_pieces(&bands, &lo, &hi) {
                // The hull already extends into the event brackets, so every refined corner lies
                // inside the certified span; no further padding (over-reach walks the fit into the
                // cutter's √-branch endpoints, where the oracle rightly declines).
                let raw = Interval {
                    lo: span_lo,
                    hi: span_hi,
                };
                let window = window_for(ri, label.0, crate::resolve::wall_of(label), &raw);
                let walls = part.ops[label.0]
                    .1
                    .walls()
                    .map_err(|_| RErr::Fault(PartFault::CutUnresolved { op: label.0 }))?;
                let wall = &walls[crate::resolve::wall_of(label)];
                // **The fit ladder** (the hole-loop inset doctrine, on the boundary path). A
                // quadric wall's rail exists only between its tangent rulings and blows up at
                // them, so the span is clamped to the window less an inset — and how much inset a
                // certifiable fit needs is a property of the sheet, not a constant: the drawing's
                // R 0.3 root fillet certifies at `ε 0.12` behind a `1/200` inset on the base pass
                // and reads `ε 9.2` behind the same inset on the offset pass. Escalating the
                // inset and the subdivision together trades boundary *reach* for certifiability,
                // which is sound twice over: the stretch given up lies against a tangent ruling
                // the splice's traced tail carries anyway, and `covered` still refuses if a
                // surviving segment turns out to need it. Rung 1 is exactly the old single
                // attempt, so every fit that certified before certifies identically; two rungs
                // because a middle one was measured useless (a 1/48 inset alone moved the four
                // fillet fits from ε ≈ 12 to ε ≈ 10, while 1/16 with doubled subdivision took
                // them to ε ≈ 0.02–0.04 — and every loose rung costs a full certification).
                const RUNGS: [(i128, usize); 2] = [(200, 1), (16, 2)];
                let rungs: &[(i128, usize)] = if window.is_some() {
                    &RUNGS
                } else {
                    &RUNGS[..1]
                };
                let mut fitted: Option<(RatFunc<B>, Rat<B>, Interval<B>)> = None;
                // The reason to report if no rung certifies: the tightest loose bound seen
                // (refinable), else the refusal.
                let mut reason: Result<Rat<B>, PartFault> =
                    Err(PartFault::CutUnresolved { op: label.0 });
                for (den, subx) in rungs {
                    let span = match &window {
                        // The inset is a stand-off from a **tangent ruling** — the √-branch
                        // endpoint the fit cannot follow. A window end that is the band's own edge
                        // is no such thing: the branch is as smooth there as anywhere, and
                        // insetting only shortens the rail short of the σ a derived end needs.
                        Some(w) => {
                            let inset = w.span.hi.sub(&w.span.lo).div(&Rat::from_i128(*den));
                            let t1 = match w.lo_is_tangent {
                                true => w.span.lo.add(&inset),
                                false => w.span.lo.clone(),
                            };
                            let t2 = match w.hi_is_tangent {
                                true => w.span.hi.sub(&inset),
                                false => w.span.hi.clone(),
                            };
                            Interval {
                                lo: rmax(&raw.lo, &t1),
                                hi: rmin(&raw.hi, &t2),
                            }
                        }
                        None => raw.clone(),
                    };
                    if span.lo.cmp(&span.hi) != Ordering::Less {
                        continue;
                    }
                    // A narrow off-origin span is ill-conditioned in the monomial basis (the
                    // G2/notch finding) — cap the fit degree there.
                    let narrow = span
                        .hi
                        .sub(&span.lo)
                        .mul(&Rat::from_i128(4))
                        .cmp(&domain_width)
                        == Ordering::Less;
                    let fit = RailFit {
                        degree: if narrow && fit_base.degree > 3 {
                            3
                        } else {
                            fit_base.degree
                        },
                        subdiv: fit_base.subdiv * subx,
                        ..fit_base
                    };
                    let pick = match label.1 {
                        BranchSide::Lower => RootPick::Lower,
                        BranchSide::Upper | BranchSide::Plane => RootPick::Upper,
                        // `upper` says which end of the cutter's **shadow** this is — not which
                        // root of the µ̂-quadratic. The two coincide only when that quadratic opens
                        // *upward*, so that "inside the cutter" is the interval **between** its
                        // roots. Every cylinder is that case, which is why the identity held until
                        // a cone wall turned up.
                        //
                        // A wall whose ruling meets it twice on one side has `a < 0`: inside is
                        // then the *complement* of the root interval, so a shadow piece's **lower**
                        // end is the quadratic's **upper** root and vice versa. Reading the sign of
                        // `a` is what makes the resolver's convention and the oracle's agree;
                        // without it the oracle traces the far branch, and `cut_fit` reports it as
                        // `NappeCrossed` — the fitted rail really is off on the mirror nappe, so
                        // the refusal is right and the cause is here.
                        //
                        // Only the *search* branch depends on this, so a midpoint sample settles
                        // it: a wrong guess costs a refusal from `cut_fit`, never a wrong
                        // `Verified`.
                        BranchSide::Wall(_, upper) => {
                            let opens_up = mu_form_opens_up(&built.charts[ri], wall, &span);
                            if upper == opens_up {
                                RootPick::Upper
                            } else {
                                RootPick::Lower
                            }
                        }
                    };
                    match certified_rail_surface(
                        &built.charts[ri],
                        wall,
                        pick,
                        &span,
                        fit,
                        &part.clearance,
                        &part.cfg,
                    ) {
                        Verdict::Verified((mu, e)) => {
                            // **The fidelity escalation.** `Verified` is the DRC bar — but a rail
                            // whose certified tube is a sizable fraction of its own µ̂ sweep still
                            // draws the feature wrong: every distance stays inside `e` while the
                            // fit's end SLOPE is tens of degrees off (measured: a fillet rail
                            // Verified at ε 1.59 met its own traced tail 22° off-direction at the
                            // splice handoff, and a cap rail at ε 0.109 on a 0.42 sweep — just
                            // over a quarter — met its tail 29° off with a 50 µm end residual;
                            // its mirror pass at ε 0.218 escalated and joined cleanly, which is
                            // what places the bar at a sixth of the sweep rather than a half). So
                            // a loose Verified keeps climbing for tightness, keeping the earlier
                            // result as the floor: nothing that certifies today can stop
                            // certifying, a later rung is only adopted if it certifies tighter.
                            let keep = match &fitted {
                                Some((_, prev, _)) => e.cmp(prev) == Ordering::Less,
                                None => true,
                            };
                            if keep {
                                fitted = Some((mu, e, span));
                            }
                            let (mu_b, e_b, span_b) =
                                fitted.as_ref().expect("just kept or already held");
                            let loose = match (mu_b.eval(&span_b.lo), mu_b.eval(&span_b.hi)) {
                                (Some(a), Some(b)) => {
                                    let sweep = if a.cmp(&b) == Ordering::Less {
                                        b.sub(&a)
                                    } else {
                                        a.sub(&b)
                                    };
                                    e_b.mul(&Rat::from_i128(6)).cmp(&sweep) == Ordering::Greater
                                }
                                _ => false,
                            };
                            if !loose {
                                break;
                            }
                        }
                        Verdict::Unresolved(e) => {
                            reason = match reason {
                                Ok(prev) => Ok(rmin(&prev, &e)),
                                Err(_) => Ok(e),
                            };
                        }
                        Verdict::Refuted(_) => {}
                    }
                }
                match fitted {
                    Some((mu, e, span)) => {
                        eps = rmax(&eps, &e);
                        pieces.push(RailPiece {
                            label,
                            region: ri,
                            mu,
                            span,
                        });
                    }
                    None => deferred.push((label, reason)),
                }
            }
        }
    }

    // — 3. Refine the run corners on the fitted rails (per changed side). —
    //
    // A corner is normally where the two fitted rails **cross**, refined by exact bisection. A
    // **flank crossing** is the exception: at least one rail's window ends at a tangent ruling
    // and the graphs never cross at all — bisecting a root that does not exist lands on an
    // arbitrary midpoint outside one rail's certificate, which `covered` then (correctly) refuses.
    // There the corner is a [`Corner::Gap`] and a [`Splice`] owns the stretch between the two
    // certificates.
    let mut upper_corners: Vec<Corner<B>> = Vec::new();
    let mut lower_corners: Vec<Corner<B>> = Vec::new();
    let mut upper_splices: Vec<Splice<B>> = Vec::new();
    let mut lower_splices: Vec<Splice<B>> = Vec::new();
    for i in 0..runs.len() - 1 {
        let (a, b) = (&runs[i].hi, &runs[i + 1].lo);
        let mid = a.add(b).mul(&Rat::new(1, 2));
        let refine = |left: Label, right: Label| -> Rat<B> {
            let ri = bands
                .iter()
                .position(|band| {
                    band.lo.cmp(&mid) != Ordering::Greater && mid.cmp(&band.hi) != Ordering::Greater
                })
                .unwrap_or(0);
            let corner = match (
                find_piece(&pieces, left, ri, &mid),
                find_piece(&pieces, right, ri, &mid),
            ) {
                (Some(l), Some(r)) => {
                    // Where the two fitted rails **cross** — or, when they never do, where they
                    // come **closest**. Two walls meeting tangentially (the drawing's rim against
                    // its own root fillet) have rails that touch at a double root of their
                    // difference: no sign change, so the crossing bisection finds nothing, and the
                    // old midpoint fallback silently parked the corner up to half a sample cell
                    // from the true contact — certified on both rails, wrong place, and a kink in
                    // an outline the drawing draws G1. The difference's σ-derivative *does* cross
                    // zero at that touch, so the closest approach is bisected the same way; the
                    // midpoint remains only for the pair no meeting point of any order exists in
                    // the gap for.
                    let dmu = l.mu.sub(&r.mu);
                    bisect_root(&dmu, a, b, 60)
                        .or_else(|| bisect_root(&dmu.derivative(), a, b, 60))
                        .unwrap_or_else(|| mid.clone())
                }
                _ => mid.clone(),
            };
            if snap_corners {
                snap30(&corner)
            } else {
                corner
            }
        };
        let corner_of = |left: Label,
                         right: Label,
                         splices: &mut Vec<Splice<B>>,
                         eps: &mut Rat<B>|
         -> Result<Corner<B>, RErr<B>> {
            if left == right {
                return Ok(Corner::At(mid.clone()));
            }
            if let Some(sp) = flank_splice(
                part,
                built,
                &bands,
                &disc_roots,
                &pieces,
                left,
                right,
                a,
                b,
                traced_splices,
            )? {
                let corner = Corner::Gap(sp.edge_a.clone(), sp.edge_b.clone());
                *eps = rmax(eps, &sp.eps);
                splices.push(sp);
                return Ok(corner);
            }
            Ok(Corner::At(refine(left, right)))
        };
        let up = corner_of(
            runs[i].upper,
            runs[i + 1].upper,
            &mut upper_splices,
            &mut eps,
        )?;
        upper_corners.push(up);
        let lo = corner_of(
            runs[i].lower,
            runs[i + 1].lower,
            &mut lower_splices,
            &mut eps,
        )?;
        lower_corners.push(lo);
    }

    // Fold the runs into per-side chain segments (from, to, label) covering the domain. A `Gap`
    // corner ends one segment at the left rail's certified edge and starts the next at the right
    // rail's — the splice owns what lies between.
    let side_segments = |corners: &[Corner<B>], label_of: &dyn Fn(usize) -> Label| {
        let mut segs: Vec<(Rat<B>, Rat<B>, Label)> = Vec::new();
        for (i, _) in runs.iter().enumerate() {
            let from = if i == 0 {
                domain.lo.clone()
            } else {
                match &corners[i - 1] {
                    Corner::At(s) => s.clone(),
                    Corner::Gap(_, edge_b) => edge_b.clone(),
                }
            };
            let to = if i + 1 == runs.len() {
                domain.hi.clone()
            } else {
                match &corners[i] {
                    Corner::At(s) => s.clone(),
                    Corner::Gap(edge_a, _) => edge_a.clone(),
                }
            };
            match segs.last_mut() {
                Some(last) if last.2 == label_of(i) => last.1 = to,
                _ => segs.push((from, to, label_of(i))),
            }
        }
        segs
    };
    let mut upper_segs = side_segments(&upper_corners, &|i| runs[i].upper);
    let mut lower_segs = side_segments(&lower_corners, &|i| runs[i].lower);

    // — 3′. Smooth-pinch ends become turn arcs. —
    //
    // Where a derived σ-end is one quadric wall's tangent ruling, the outermost segment of *each*
    // chain is that wall's two branches running into it — and a graph fit cannot follow them there
    // (unbounded slope), which is what `RailSpanShort` refuses. Both segments come out and one
    // `tangent_turn_arc` goes in: from the upper junction, through the tangent, back to the lower
    // one. The remaining rails then sit well inside their windows, so the coverage check below
    // passes for the reason it should rather than by being skipped.
    let mut end_arcs: [Option<Vec<develop::pcurve::PCurve<B>>>; 2] = [None, None];
    let both_pinched = structure
        .ends
        .iter()
        .all(|e| matches!(e, crate::resolve::SigmaEnd::Closed { pinch: true, .. }));
    // The contour's two branches meeting at one wall: what the outermost segment of each chain must
    // be for a turn arc to belong there at all.
    let one_wall = |a: Label, b: Label| a.0 == b.0 && wall_of(a) == wall_of(b);

    // **(i) The contour bounds one whole side.** That chain is a *single* segment, so there is no
    // second junction on it: the two tangents are joined by one continuous run of contour boundary
    // and the answer is ONE arc wrapping both, not two per-end arcs. It takes the whole single chain
    // and both of the other chain's contour segments, leaving the other chain's middle rail — so the
    // boundary is that rail out, and the arc all the way back.
    if both_pinched
        && lower_segs.len() == 1
        && upper_segs.len() >= 3
        && one_wall(upper_segs[0].2, lower_segs[0].2)
        && one_wall(upper_segs[upper_segs.len() - 1].2, lower_segs[0].2)
    {
        let op = lower_segs[0].2.0;
        if matches!(part.ops[op].0, OpKind::Intersect)
            && let Some(cut) = contour_loop(part, built, &bands[0], op, wall_of(lower_segs[0].2))?
        {
            eps = rmax(&eps, &cut.eps);
            // Leave the upper branch at its σ_hi junction, wrap both tangents, rejoin at σ_lo.
            let from = upper_segs[upper_segs.len() - 1].0.clone();
            let to = upper_segs[0].1.clone();
            let arc = develop::cut::tangent_turn_arc(&cut, &from, true, &to, true)
                .ok_or(RErr::Fault(PartFault::CutUnresolved { op }))?;
            upper_segs.pop();
            upper_segs.remove(0);
            lower_segs.clear();
            end_arcs[1] = Some(arc);
        }
    }

    // **(ii) The contour takes over near each end.** One arc per end, each replacing that end's
    // outermost segment on **both** chains.
    for side in [1usize, 0] {
        let upper_end = side == 1;
        // Already taken by (i), not a smooth pinch, or a chain with nothing to give at both ends —
        // the last of which is (i)'s shape when (i) declined it, and refuses below by name rather
        // than producing half a boundary here.
        if end_arcs[side].is_some()
            || !matches!(
                structure.ends[side],
                crate::resolve::SigmaEnd::Closed { pinch: true, .. }
            )
            || upper_segs.len() < 2
            || lower_segs.len() < 2
        {
            continue;
        }
        let (ui, li) = if upper_end {
            (upper_segs.len() - 1, lower_segs.len() - 1)
        } else {
            (0, 0)
        };
        let (ul, ll) = (upper_segs[ui].2, lower_segs[li].2);
        if !one_wall(ul, ll) || !matches!(part.ops[ul.0].0, OpKind::Intersect) {
            continue;
        }
        let op = ul.0;
        let Some(cut) = contour_loop(part, built, &bands[0], op, wall_of(ul))? else {
            continue;
        };
        eps = rmax(&eps, &cut.eps);
        // The junctions: where each chain hands the boundary to the contour.
        let (from, to) = if upper_end {
            (upper_segs[ui].0.clone(), lower_segs[li].0.clone())
        } else {
            (lower_segs[li].1.clone(), upper_segs[ui].1.clone())
        };
        // One tangent per end: the arc leaves the upper branch and rejoins the lower at a σ_hi end,
        // and the other way round at σ_lo.
        let arc = develop::cut::tangent_turn_arc(&cut, &from, upper_end, &to, !upper_end)
            .ok_or(RErr::Fault(PartFault::CutUnresolved { op }))?;
        upper_segs.remove(ui);
        lower_segs.remove(li);
        end_arcs[side] = Some(arc);
    }

    // A rail may only be **used** where it was **certified**. The window clamp above shortens a fit
    // span to the wall's disc-positive window, inset a hair — and before AUTH.3 that was always
    // wider than the boundary needed, because the outer boundary never approached a tangent ruling;
    // only interior holes did, and p-curves were built for them (PC.3). A *derived* σ-end can sit
    // exactly on one. Evaluating a fitted graph past its certified span there is extrapolation into
    // a √-branch with unbounded slope, and the ε this function reports would be a bound on a
    // stretch of rail the geometry does not use — a certificate pointing away from the artifact.
    // Refused by name instead; §12.4's p-curve end is what lifts it.
    let covered = |segs: &[(Rat<B>, Rat<B>, Label)]| -> Result<(), RErr<B>> {
        for (a, b, label) in segs {
            for (plo, phi, ri) in span_pieces(&bands, a, b) {
                // No piece means the fit was recorded rather than raised (step 2). This segment
                // survived step 3′, so the boundary really does need that rail: raise the reason
                // the fit gave — a loose one stays refinable, a refusal stays a refusal.
                let Some(piece) = find_piece(&pieces, *label, ri, &plo) else {
                    return Err(match deferred.iter().find(|(l, _)| l == label) {
                        Some((_, Ok(e))) => RErr::Loose(e.clone()),
                        Some((_, Err(f))) => RErr::Fault(*f),
                        None => RErr::Fault(PartFault::CutUnresolved { op: label.0 }),
                    });
                };
                if plo.cmp(&piece.span.lo) == Ordering::Less
                    || piece.span.hi.cmp(&phi) == Ordering::Less
                {
                    // A piece that does not reach the segment may be `find_piece`'s last-resort
                    // fallback — the same label's rail from the chart's *other pass* (#293) —
                    // standing in for a fit that was recorded rather than raised. The recorded
                    // reason is the honest refusal then: a loose fit stays refinable, and
                    // `RailSpanShort` keeps naming the genuinely short certificate.
                    return Err(match deferred.iter().find(|(l, _)| l == label) {
                        Some((_, Ok(e))) => RErr::Loose(e.clone()),
                        Some((_, Err(f))) => RErr::Fault(*f),
                        None => RErr::Fault(PartFault::RailSpanShort { op: label.0 }),
                    });
                }
            }
        }
        Ok(())
    };
    covered(&upper_segs)?;
    covered(&lower_segs)?;

    Ok(Boundary {
        pieces,
        upper_segs,
        lower_segs,
        upper_splices,
        lower_splices,
        end_arcs,
        eps,
    })
}

/// One certified interior cut: which op made it, which region's chart carries it, and the loop.
///
/// The region travels with the loop because `structure.holes` already knows it — a consumer that
/// has to *evaluate* the chart there (the diagnostic cutter body lifts the footprint back to 3-D)
/// would otherwise have to search the σ-bands for it, and a search would be a second, weaker
/// answer to a question the resolver has already decided.
pub(crate) struct CertifiedHole<B: Backend> {
    /// The material op that cut it — an index into the part's ops.
    pub op: usize,
    /// The region whose chart the loop lives on — an index into `BuiltRegions::charts`.
    pub region: usize,
    /// The certified boundary loop, in domain coordinates `(σ, µ̂)`.
    pub boundary: HoleLoop<B>,
}

/// Certify each hole op's loop (extent, both branch rails, micro-caps).
///
/// A hole's window is a **narrow span**, so the fit degree caps at 3 (the G2 narrow-span
/// finding: higher degrees are Vandermonde-catastrophic off-origin, and the certified ε refines
/// by `subdiv`, not degree). The ladder escalates the tangent inset (a thin inset leaves the
/// near-tangent `∂s/∂µ̂ → 0` region inside the fit span, which blows the certified bound on a
/// fast-turning chart) and then the subdivision, starting from the user's knobs; the first
/// verified rung wins, and a dry ladder reports the tightest ε reached (fail-closed).
fn certify_holes<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: &Structure<B>,
    segments: usize,
) -> Result<Vec<CertifiedHole<B>>, RErr<B>> {
    let mut out = Vec::with_capacity(structure.holes.len());
    for (op, ri, window) in &structure.holes {
        let (op, ri) = (*op, *ri);
        // Scan just past the window so its two tangents are clean sign changes, but stay clear
        // of any neighboring window (the wide-gore multi-window case).
        let pad = window.hi.sub(&window.lo).mul(&Rat::new(1, 16));
        let span = Interval {
            lo: rmax(&window.lo.sub(&pad), &part.regions[ri].band.lo),
            hi: rmin(&window.hi.add(&pad), &part.regions[ri].band.hi),
        };
        let cutter = &part.ops[op].1;
        let walls = cutter
            .walls()
            .map_err(|_| RErr::Fault(PartFault::CutUnresolved { op }))?;
        // One wall is its own boundary, and its two branches come off one µ̂-quadratic. Several
        // walls have no such quadratic: which one bounds the hole changes along the loop, at every
        // profile corner, so the boundary is read from the cutter's own fill rule instead.
        let verdict = match (walls.len(), cutter) {
            (1, _) => match surface_hole_loop(
                &built.charts[ri],
                &walls[0],
                &span,
                &part.clearance,
                &part.cfg,
                segments,
            ) {
                // One wall is its own boundary and gives one loop; the plural shape is the general
                // one, so the single-surface path joins it rather than being special-cased around.
                Verdict::Verified(h) => Verdict::Verified(vec![h]),
                Verdict::Unresolved(e) => Verdict::Unresolved(e),
                Verdict::Refuted(f) => Verdict::Refuted(f),
            },
            (_, Cutter::Extrude(e)) => {
                let cast = e
                    .cast()
                    .map_err(|_| RErr::Fault(PartFault::CutUnresolved { op }))?;
                let chart = &built.charts[ri];
                let zero = Rat::from_i128(0);
                // The same footprint the resolver read — `Cast::contains` on the chart's own
                // surface point — so the certified loop bounds the region the structure was
                // resolved from, not a stricter one. (The authored nappe is enforced downstream:
                // a loop reaching the mirror nappe is `NappeCrossed`, a refusal.)
                shadow_hole_loops(
                    chart,
                    &walls,
                    |sigma: &Rat<B>, mu: &Rat<B>| {
                        let p = chart.surface(mu, &zero).eval(sigma)?;
                        cast.contains(&p, &e.profile)
                    },
                    &span,
                    &part.clearance,
                    &part.cfg,
                    segments,
                )
            }
            // Unreachable today: only an extrusion has several walls.
            _ => return Err(RErr::Fault(PartFault::CutUnresolved { op })),
        };
        match verdict {
            Verdict::Verified(hs) => out.extend(hs.into_iter().map(|h| CertifiedHole {
                op,
                region: ri,
                boundary: h,
            })),
            Verdict::Unresolved(e) => return Err(RErr::Loose(e)),
            Verdict::Refuted(develop::cut::CutFitFault::PoleInEval) => {
                return Err(RErr::Fault(PartFault::Pole));
            }
            // A deliberate scope refusal, not a looseness: say which, so the author learns that the
            // profile is the problem rather than the resolution. Since AUTH.2c the tracer reads a
            // non-convex footprint directly, so this is the **ring** — a hole with a hole of its
            // own, which would leave an island of material floating free.
            Verdict::Refuted(develop::cut::CutFitFault::ShadowNested) => {
                return Err(RErr::Fault(PartFault::ProfileNotSimple { op }));
            }
            Verdict::Refuted(_) => return Err(RErr::Fault(PartFault::CutUnresolved { op })),
        }
    }
    Ok(out)
}

/// One cutter's **traced footprint**: where it actually reached, as a closed polygon in the
/// domain coordinates `(σ, µ̂)` of the chart that carries it.
pub(crate) struct Footprint<B: Backend> {
    /// The material op that cut it — an index into the part's ops.
    pub op: usize,
    /// The region whose chart the polygon lives on — an index into `BuiltRegions::charts`.
    pub region: usize,
    /// The loop's vertices in traversal order, the first not repeated at the end.
    pub poly: Vec<(Rat<B>, Rat<B>)>,
    /// The loop's certified distance bound.
    pub eps: Rat<B>,
}

/// Every hole op's certified footprint, as the `(σ, µ̂)` polygons the solid path already cuts with.
///
/// This is [`certify_holes`] read for its *geometry* rather than for the flat pattern: the same
/// certified loops, put through the same [`hole_poly`] the solid builder's general hole channel
/// takes. Sharing that converter is the point — a diagnostic drawn from a second, parallel sampler
/// would answer a slightly different question than the one the part was actually built from, and a
/// diagnostic that disagrees with the build for reasons of its own is worse than none.
///
/// In particular `hole_poly`'s sub-[`MIN_STEP`](export::trim) vertex merge is inherited rather than
/// re-derived: the tracer parks a pair of vertices ~10⁻⁹ apart at every cell boundary, which is
/// correct in the domain and unbuildable by any `f64` consumer — including the one this feeds.
pub(crate) fn footprints<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: &Structure<B>,
    segments: usize,
) -> Result<Vec<Footprint<B>>, RErr<B>> {
    certify_holes(part, built, structure, segments)?
        .into_iter()
        .map(|h| {
            let poly = hole_poly(&h.boundary).ok_or(RErr::Fault(PartFault::LoopBroken))?;
            Ok(Footprint {
                op: h.op,
                region: h.region,
                poly,
                eps: h.boundary.eps,
            })
        })
        .collect()
}

/// One contour wall's own footprint loop over its two tangent rulings — the object every turn arc
/// is cut out of. `None` when the wall is not a genuine quadratic or has no two tangents in the
/// band, i.e. when there is no smooth pinch to turn around.
///
/// The tangent window comes from the same `tangent_events` isolation the σ-extent was derived from,
/// so the loop this traces and the end the resolver located are brackets of the *same* roots.
fn contour_loop<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    band: &Interval<B>,
    op: usize,
    wall_ix: usize,
) -> Result<Option<develop::cut::CutLoop<B>>, RErr<B>> {
    let walls = part.ops[op]
        .1
        .walls()
        .map_err(|_| RErr::Fault(PartFault::CutUnresolved { op }))?;
    let wall = &walls[wall_ix];
    let zero = Rat::from_i128(0);
    let form = match develop::cut::cut_mu_form(&built.charts[0], wall, &zero) {
        Some(f) if !f.a.is_zero() => f,
        _ => return Ok(None),
    };
    let brackets = develop::cut::tangent_events(&form, band, &crate::resolve::tangent_tol())
        .map_err(|_| RErr::Fault(PartFault::CutUnresolved { op }))?;
    if brackets.len() < 2 {
        return Ok(None);
    }
    let window = Interval {
        lo: brackets[0].hi.clone(),
        hi: brackets[brackets.len() - 1].lo.clone(),
    };
    match develop::cut::quadric_cut_loop(
        &built.charts[0],
        wall,
        &window,
        &zero,
        part.segments,
        &part.clearance,
        &part.cfg,
    ) {
        Verdict::Verified(l) => Ok(Some(l)),
        Verdict::Unresolved(e) => Err(RErr::Loose(e)),
        Verdict::Refuted(_) => Err(RErr::Fault(PartFault::CutUnresolved { op })),
    }
}

/// The op whose **own footprint loop is the whole outer boundary** — every run bounded above and
/// below by the same intersect, and both derived σ-ends closed inside the band.
///
/// This is the shape a graph chain cannot express and a traced loop can
/// (`docs/cutter-extrude-design.md` §12.4). At a tangent ruling a quadric wall's two branches meet
/// with **unbounded slope**, so `certified_rail_surface` clamps its fit away from it and the chain
/// runs out of certificate (`PartFault::RailSpanShort`). The traced loop has no such trouble: it is
/// parametric, passes *through* the tangent, and is exactly what PC.3 built for interior holes —
/// the same construction, used as an outline rather than as a hole.
///
/// Each condition is load-bearing rather than defensive:
///
/// - **one region**, because a loop spanning a region join would need the anchor frames threaded
///   through the tracer, which is not this slice;
/// - **one op bounds both sides everywhere**, so the boundary really is that cutter's own
///   footprint. Which *wall* bounds it may change along the loop — that is what a profile corner
///   is — so this deliberately does **not** ask for one wall, and [`contour_outline`] reads the
///   boundary from the cutter's fill rule exactly as a multi-wall hole does;
/// - **at least one genuine quadratic wall** (`a ≢ 0`). This is a capability test, not a shape one:
///   an all-affine contour is carried *exactly* by the graph chains — `plane_cut_rail` needs no fit,
///   its corners are transverse crossings of two straight rails, and the boundary certifies at
///   `ε = 0`. Tracing it instead would replace exact rails with chords, which is strictly worse. A
///   quadric wall is what a graph cannot reach, so it is what earns the traced loop.
fn sole_contour<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: &Structure<B>,
) -> Option<usize> {
    use crate::part::OpKind;
    use crate::resolve::SigmaEnd;
    if part.regions.len() != 1 {
        return None;
    }
    let first = structure.runs.first()?;
    let op = first.lower.0;
    if !matches!(part.ops[op].0, OpKind::Intersect) {
        return None;
    }
    if !structure
        .runs
        .iter()
        .all(|r| r.lower.0 == op && r.upper.0 == op)
    {
        return None;
    }
    // Both ends closed inside the band rather than reaching it.
    if !structure
        .ends
        .iter()
        .all(|e| matches!(e, SigmaEnd::Closed { .. }))
    {
        return None;
    }
    let walls = part.ops[op].1.walls().ok()?;
    let zero = Rat::from_i128(0);
    walls
        .iter()
        .any(|w| {
            develop::cut::cut_mu_form(&built.charts[0], w, &zero).is_some_and(|f| !f.a.is_zero())
        })
        .then_some(op)
}

/// The traced footprint loop of the contour that bounds the part — [`sole_contour`]'s op, as one
/// closed `(σ, µ̂)` boundary.
///
/// The dispatch is [`certify_holes`]' verbatim, because it is the same question asked of the same
/// cutter: **one** wall is its own boundary and its two branches come off one µ̂-quadratic, while
/// **several** have no such quadratic — which wall bounds the material changes at every profile
/// corner, so the boundary is read from the cutter's own fill rule instead. A rounded outline is
/// the case that forces it: its corner arcs are short quadric walls whose entire disc-positive
/// window sits within `10⁻⁴` of a tangent ruling, where a degree-3 monomial fit is singular — the
/// oracle declines, correctly, and no amount of clamping helps because the whole window is the
/// √-branch.
///
/// Exactly **one** loop, or the contour's footprint is in pieces and the part is disconnected —
/// which the resolver would already have refused, so this is the realizer agreeing rather than a
/// second opinion.
fn contour_outline<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    op: usize,
    segments: usize,
) -> Result<HoleLoop<B>, RErr<B>> {
    let walls = part.ops[op]
        .1
        .walls()
        .map_err(|_| RErr::Fault(PartFault::CutUnresolved { op }))?;
    let span = part.regions[0].band.clone();
    let chart = &built.charts[0];
    let verdict = match (walls.len(), &part.ops[op].1) {
        (1, _) => match surface_hole_loop(
            chart,
            &walls[0],
            &span,
            &part.clearance,
            &part.cfg,
            segments,
        ) {
            Verdict::Verified(h) => Verdict::Verified(vec![h]),
            Verdict::Unresolved(e) => Verdict::Unresolved(e),
            Verdict::Refuted(f) => Verdict::Refuted(f),
        },
        (_, Cutter::Extrude(e)) => {
            let cast = e
                .cast()
                .map_err(|_| RErr::Fault(PartFault::CutUnresolved { op }))?;
            let zero = Rat::from_i128(0);
            shadow_hole_loops(
                chart,
                &walls,
                |sigma: &Rat<B>, mu: &Rat<B>| {
                    let p = chart.surface(mu, &zero).eval(sigma)?;
                    cast.contains(&p, &e.profile)
                },
                &span,
                &part.clearance,
                &part.cfg,
                segments,
            )
        }
        // Unreachable today: only an extrusion has several walls.
        _ => return Err(RErr::Fault(PartFault::CutUnresolved { op })),
    };
    match verdict {
        Verdict::Verified(mut hs) if hs.len() == 1 => Ok(hs.pop().expect("length checked")),
        Verdict::Verified(_) => Err(RErr::Fault(PartFault::DisconnectedRegion)),
        Verdict::Unresolved(e) => Err(RErr::Loose(e)),
        Verdict::Refuted(develop::cut::CutFitFault::PoleInEval) => {
            Err(RErr::Fault(PartFault::Pole))
        }
        Verdict::Refuted(develop::cut::CutFitFault::ShadowNested) => {
            Err(RErr::Fault(PartFault::ProfileNotSimple { op }))
        }
        Verdict::Refuted(_) => Err(RErr::Fault(PartFault::CutUnresolved { op })),
    }
}

/// Certify the resolved structure into the [`FlatPattern`] (see the module docs).
pub(crate) fn flat_pattern<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: Structure<B>,
) -> Verdict<FlatPattern<B>, PartFault, Rat<B>> {
    use core::cmp::Ordering;
    let bands: Vec<Interval<B>> = part.regions.iter().map(|r| r.band.clone()).collect();
    // The **derived** extent, the same one `certify_boundary` builds its segments over. Reading it
    // off `bands` here instead was the whole of a `Pole`: the caps were evaluated at the authored
    // band ends while every rail piece was fitted over the contour's own footprint, so a rail was
    // asked for its value a quarter-turn away from where it exists.
    let domain = structure.domain.clone();

    // — 4′. The contour IS the boundary: one traced loop, no chain. —
    //
    // When the part is exactly what one quadric wall keeps, its outer boundary is that wall's own
    // footprint loop, and the two derived σ-ends are its tangent rulings. A chain of graph rails
    // cannot reach those (the branches meet with unbounded slope, so the fit is clamped away and
    // `RailSpanShort` refuses); the traced loop passes *through* them because it is parametric in
    // its own parameter rather than a graph over σ. `unroll_trim_loop` already takes such arcs —
    // this is PC.3's construction used as an outline instead of as a hole.
    if let Some(op) = sole_contour(part, built, &structure) {
        let hole = bail!(contour_outline(part, built, op, part.segments));
        let outline = match unroll_trim_loop(&built.pw, &hole.arcs, &part.cfg, &part.clearance) {
            Verdict::Verified(o) => o,
            Verdict::Unresolved(e) => {
                return Verdict::Unresolved(e);
            }
            Verdict::Refuted(UnrollFault::PoleInEval) => return Verdict::Refuted(PartFault::Pole),
            Verdict::Refuted(_) => return Verdict::Refuted(PartFault::LoopBroken),
        };
        let eps_all = rmax(&hole.eps, &outline.eps);
        return pattern_from_outline(part, built, structure, outline, eps_all);
    }

    let boundary = bail!(certify_boundary(
        part, built, &structure, part.fit, false, true
    ));
    let mut eps_all = boundary.eps.clone();

    // — 4. Assemble the one general boundary loop. —
    let eval = |label: Label, sigma: &Rat<B>| rail_at(&boundary.pieces, &bands, label, sigma);
    let mut arcs: Vec<BoundaryArc<B>> = Vec::new();
    // The lower end: a turn arc where the material pinches at a tangent ruling, otherwise the
    // ruling cap it always was. Both take the boundary from the lower side to the upper one.
    // A turn arc joins the chain the same way a rail does — through a micro-cap. The arc starts at
    // the wall's **true** branch value and the rail it follows ends at that rail's **fitted** one,
    // so they differ by the fit's own ε at the same σ. That gap is a ruling segment, which is what
    // a `Cap` is; the chain assembly has always closed rail-to-rail junctions this way, and the
    // arcs need it for the same reason rather than a new one.
    let push_turn = |arcs: &mut Vec<BoundaryArc<B>>, side: usize| -> Option<()> {
        let curves = boundary.end_arcs[side].as_ref()?;
        let head = curves.first()?;
        let [sa, ma] = head.eval(&head.domain.lo)?;
        if let Some(prev) = last_end(arcs)
            && prev.1.cmp(&ma) != Ordering::Equal
        {
            arcs.push(BoundaryArc::Cap {
                sigma: sa,
                mu_start: prev.1.clone(),
                mu_end: ma,
            });
        }
        for curve in curves {
            // **One chord per traced piece**, which is what `segments` means on a `Curve`: how
            // finely to re-sample *this* piece, not how many pieces the loop has. The tracer
            // already spent `part.segments` on the piece count, so asking for `part.segments`
            // sub-samples of each piece multiplies the two — the whole-side arc is most of the
            // loop, and it emitted `6386` outline points against `192` for the same contour
            // traced as a sole boundary, `175s` to develop against `3s` (#281). `export::trim`
            // has mapped a traced piece to `segments: 1` since PC.4; this is the same object.
            arcs.push(BoundaryArc::Curve {
                curve: curve.clone(),
                segments: 1,
            });
        }
        Some(())
    };
    // A cap is only needed where two *chains* meet. A turn arc already carries the boundary from one
    // side to the other, and where the contour bounds a whole side that chain is **empty** — the arc
    // is the entire path between the other chain's two ends, so there is nothing here to cap.
    if boundary.end_arcs[0].is_some() {
        if push_turn(&mut arcs, 0).is_none() {
            return Verdict::Refuted(PartFault::LoopBroken);
        }
    } else if !boundary.lower_segs.is_empty() && !boundary.upper_segs.is_empty() {
        let (lo0, up0) = match (
            eval(boundary.lower_segs[0].2, &domain.lo),
            eval(boundary.upper_segs[0].2, &domain.lo),
        ) {
            (Some(a), Some(b)) => (a, b),
            _ => return Verdict::Refuted(PartFault::Pole),
        };
        arcs.push(BoundaryArc::Cap {
            sigma: domain.lo.clone(),
            mu_start: lo0,
            mu_end: up0,
        });
    }
    // Each chain: rails split at region joins, micro-caps at junctions — and where two segments
    // are joined by a flank crossing, the splice's traced route between them. A splice joins the
    // chain the way a turn arc does: its head starts at the wall's **true** branch value and the
    // rail it follows ends at its **fitted** one, so a micro-cap (the ε-wide ruling gap every
    // junction has) closes the difference; within the splice the pieces chain head-to-tail
    // exactly.
    let push_splice =
        |arcs: &mut Vec<BoundaryArc<B>>, sp: &Splice<B>, forward: bool| -> Result<(), PartFault> {
            let list: Vec<develop::pcurve::PCurve<B>> = if forward {
                sp.curves.clone()
            } else {
                sp.curves
                    .iter()
                    .rev()
                    .map(|c| develop::cut::rev_chord(c).ok_or(PartFault::LoopBroken))
                    .collect::<Result<_, _>>()?
            };
            if let Some(head) = list.first() {
                let [sa, ma] = head.eval(&head.domain.lo).ok_or(PartFault::Pole)?;
                if let Some(prev) = last_end(arcs)
                    && prev.1.cmp(&ma) != Ordering::Equal
                {
                    arcs.push(BoundaryArc::Cap {
                        sigma: sa,
                        mu_start: prev.1.clone(),
                        mu_end: ma,
                    });
                }
            }
            // A piece that is exactly vertical IS a cap, and one that is exactly µ̂-constant IS a
            // one-chord rail — emitted as those arc kinds, they develop exactly (the cap) or carry the
            // cheap graph chord bound (the rail), where the generic curve chord bound over the same
            // stretch cannot be tight (the connector's µ̂ span is millimetres). The translation is
            // lossless: the arcs trace the same domain points.
            for curve in list {
                let a = curve.eval(&curve.domain.lo).ok_or(PartFault::Pole)?;
                let b = curve.eval(&curve.domain.hi).ok_or(PartFault::Pole)?;
                if a[0].cmp(&b[0]) == Ordering::Equal {
                    arcs.push(BoundaryArc::Cap {
                        sigma: a[0].clone(),
                        mu_start: a[1].clone(),
                        mu_end: b[1].clone(),
                    });
                } else if a[1].cmp(&b[1]) == Ordering::Equal {
                    arcs.push(BoundaryArc::Rail {
                        mu: RatFunc::from_poly(lattice::Poly::constant(a[1].clone())),
                        sigma_start: a[0].clone(),
                        sigma_end: b[0].clone(),
                        segments: 1,
                    });
                } else {
                    arcs.push(BoundaryArc::Curve { curve, segments: 1 });
                }
            }
            Ok(())
        };
    // A rail is chordized **√-graded into a spliced end**, uniformly elsewhere. A splice's
    // handoff sits an inset short of a tangent ruling, where the rail behaves like
    // `µ̂ − µ̂_t ∝ √(σ − σ_t)` — and uniform-in-σ chords against a √-branch bunch the whole turn
    // into the last chord (measured: 53° in one chord at `segments = 8`, a visible facet on a
    // cap the drawing draws G1, on a boundary every distance certificate was blind to). Graded
    // breakpoints `end ∓ g·(k/n)²` make the chords equal-turn, exactly why the turn tails are
    // graded — and `g` a quarter of the span makes the bulk's last uniform chord and the graded
    // stretch's largest turn agree at every `segments`. The carrier is the SAME certified rail
    // polynomial; only chord placement moves, so nothing new is certified (the unroll's
    // per-chord certificates only tighten on shorter spans), and consecutive sub-rails evaluate
    // one polynomial at one shared σ, so no micro-cap forms between them.
    let push_rail = |arcs: &mut Vec<BoundaryArc<B>>,
                     mu: &RatFunc<B>,
                     start: Rat<B>,
                     end: Rat<B>,
                     grade_start: bool,
                     grade_end: bool| {
        let n = part.segments.max(2);
        if start.cmp(&end) == Ordering::Equal || (!grade_start && !grade_end) {
            arcs.push(BoundaryArc::Rail {
                mu: mu.clone(),
                sigma_start: start,
                sigma_end: end,
                segments: part.segments,
            });
            return;
        }
        // `g` is signed by the walk direction, so the same formulas serve both chains.
        let g = end.sub(&start).mul(&Rat::new(1, 4));
        let quad = |k: usize| {
            let f = Rat::new(k as i128, n as i128);
            f.mul(&f)
        };
        if grade_start {
            for k in 0..n {
                arcs.push(BoundaryArc::Rail {
                    mu: mu.clone(),
                    sigma_start: start.add(&g.mul(&quad(k))),
                    sigma_end: start.add(&g.mul(&quad(k + 1))),
                    segments: 1,
                });
            }
        }
        arcs.push(BoundaryArc::Rail {
            mu: mu.clone(),
            sigma_start: if grade_start { start.add(&g) } else { start },
            sigma_end: if grade_end { end.sub(&g) } else { end.clone() },
            segments: part.segments,
        });
        if grade_end {
            for k in (1..=n).rev() {
                arcs.push(BoundaryArc::Rail {
                    mu: mu.clone(),
                    sigma_start: end.sub(&g.mul(&quad(k))),
                    sigma_end: end.sub(&g.mul(&quad(k - 1))),
                    segments: 1,
                });
            }
        }
    };
    let push_chain = |arcs: &mut Vec<BoundaryArc<B>>,
                      segs: &[(Rat<B>, Rat<B>, Label)],
                      splices: &[Splice<B>],
                      forward: bool|
     -> Result<(), PartFault> {
        let order: Vec<usize> = if forward {
            (0..segs.len()).collect()
        } else {
            (0..segs.len()).rev().collect()
        };
        let mut entered_from_splice = false;
        for &si in &order {
            let (a, b, label) = &segs[si];
            // The splice keyed to this segment's trailing edge, if any — resolved before the
            // rails are emitted, because it also decides which rail end is graded.
            let key = if forward { b } else { a };
            let sp = splices.iter().find(|s| {
                let edge = if forward { &s.edge_a } else { &s.edge_b };
                edge.cmp(key) == Ordering::Equal
            });
            let mut region_pieces = span_pieces(&bands, a, b);
            if !forward {
                region_pieces.reverse();
            }
            let np = region_pieces.len();
            for (pi, (plo, phi, ri)) in region_pieces.into_iter().enumerate() {
                let piece =
                    find_piece(&boundary.pieces, *label, ri, &plo).ok_or(PartFault::Pole)?;
                let (start, end) = if forward {
                    (plo.clone(), phi.clone())
                } else {
                    (phi.clone(), plo.clone())
                };
                // Micro-cap joining the previous arc's end to this rail's start value.
                let v = piece.mu.eval(&start).ok_or(PartFault::Pole)?;
                if let Some(prev_end) = last_end(arcs)
                    && prev_end.1.cmp(&v) != Ordering::Equal
                {
                    arcs.push(BoundaryArc::Cap {
                        sigma: start.clone(),
                        mu_start: prev_end.1.clone(),
                        mu_end: v,
                    });
                }
                push_rail(
                    arcs,
                    &piece.mu,
                    start,
                    end,
                    entered_from_splice && pi == 0,
                    sp.is_some() && pi + 1 == np,
                );
            }
            entered_from_splice = sp.is_some();
            if let Some(sp) = sp {
                push_splice(arcs, sp, forward)?;
            }
        }
        Ok(())
    };
    if let Err(f) = push_chain(
        &mut arcs,
        &boundary.upper_segs,
        &boundary.upper_splices,
        true,
    ) {
        return Verdict::Refuted(f);
    }
    // The far end, symmetrically: the turn arc, or the cap.
    if boundary.end_arcs[1].is_some() {
        if push_turn(&mut arcs, 1).is_none() {
            return Verdict::Refuted(PartFault::LoopBroken);
        }
    } else if !boundary.lower_segs.is_empty() && !boundary.upper_segs.is_empty() {
        let lo1 = match eval(
            boundary.lower_segs[boundary.lower_segs.len() - 1].2,
            &domain.hi,
        ) {
            Some(v) => v,
            None => return Verdict::Refuted(PartFault::Pole),
        };
        if let Some(prev_end) = last_end(&arcs)
            && prev_end.1.cmp(&lo1) != Ordering::Equal
        {
            arcs.push(BoundaryArc::Cap {
                sigma: domain.hi.clone(),
                mu_start: prev_end.1.clone(),
                mu_end: lo1,
            });
        }
    }
    if let Err(f) = push_chain(
        &mut arcs,
        &boundary.lower_segs,
        &boundary.lower_splices,
        false,
    ) {
        return Verdict::Refuted(f);
    }
    // Close the loop back onto the first arc. Without a turn end the last rail and the opening cap
    // read the *same* rail at the *same* σ, so the loop shuts exactly and nothing is needed; with
    // one, the opening arc began at the wall's true branch and the closing rail ends at its fitted
    // value, the same ε-wide ruling gap every other junction has.
    if let (Some(first), Some(prev)) = (arcs.first(), last_end(&arcs)) {
        let start = match first {
            BoundaryArc::Cap {
                sigma, mu_start, ..
            } => Some((sigma.clone(), mu_start.clone())),
            BoundaryArc::Curve { curve, .. } => {
                curve.eval(&curve.domain.lo).map(|[sigma, mu]| (sigma, mu))
            }
            BoundaryArc::Rail {
                mu, sigma_start, ..
            } => mu.eval(sigma_start).map(|m| (sigma_start.clone(), m)),
        };
        match start {
            Some((sigma, mu)) if prev.1.cmp(&mu) != Ordering::Equal => {
                arcs.push(BoundaryArc::Cap {
                    sigma,
                    mu_start: prev.1.clone(),
                    mu_end: mu,
                });
            }
            None => return Verdict::Refuted(PartFault::Pole),
            _ => {}
        }
    }

    // — 5. Unroll the boundary through the connected piecewise development. —
    let outline = match unroll_trim_loop(&built.pw, &arcs, &part.cfg, &part.clearance) {
        Verdict::Verified(o) => o,
        Verdict::Unresolved(e) => {
            return Verdict::Unresolved(e);
        }
        Verdict::Refuted(UnrollFault::PoleInEval) => return Verdict::Refuted(PartFault::Pole),
        Verdict::Refuted(_) => return Verdict::Refuted(PartFault::LoopBroken),
    };
    eps_all = rmax(&eps_all, &outline.eps);
    pattern_from_outline(part, built, structure, outline, eps_all)
}

/// Steps 6–9, shared by both boundary assemblies: the interior cuts, the authored polygons, the
/// exact flat boolean with its topology-coherence gate, and the report echo.
fn pattern_from_outline<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: Structure<B>,
    outline: FlatOutline<B>,
    mut eps_all: Rat<B>,
) -> Verdict<FlatPattern<B>, PartFault, Rat<B>> {
    // — 6. Interior holes: certified loops + unroll. —
    let holes = bail!(certify_holes(
        part,
        built,
        &structure,
        (part.segments / 2).max(4),
    ));
    let mut hole_outlines: Vec<FlatOutline<B>> = Vec::new();
    for hole in &holes {
        let hole = &hole.boundary;
        eps_all = rmax(&eps_all, &hole.eps);
        let flat = match unroll_trim_loop(&built.pw, &hole.arcs, &part.cfg, &part.clearance) {
            Verdict::Verified(o) => o,
            Verdict::Unresolved(e) => {
                return Verdict::Unresolved(e);
            }
            Verdict::Refuted(UnrollFault::PoleInEval) => {
                return Verdict::Refuted(PartFault::Pole);
            }
            Verdict::Refuted(_) => return Verdict::Refuted(PartFault::LoopBroken),
        };
        eps_all = rmax(&eps_all, &flat.eps);
        hole_outlines.push(flat);
    }

    // — 7. Domain-authored holes: develop the vertices through the same connected frame. —
    let mut domain_polys: Vec<Vec<[Rat<B>; 2]>> = Vec::new();
    for poly in &part.domain_holes {
        let mut flat = Vec::with_capacity(poly.len());
        for (s, m) in poly {
            let fb = match Development::point(&built.pw, s, m, &part.cfg) {
                Some(fb) => fb,
                None => return Verdict::Refuted(PartFault::Pole),
            };
            let (x, y) = fb.center();
            flat.push([x, y]);
        }
        domain_polys.push(flat);
    }

    // — 8. The exact flat boolean + the topology coherence gate. —
    // Flat-authored holes are already flat data: they cut directly (their fold-back is the
    // solid evaluator's job). The coherence gate covers them too — a flat hole outside the
    // pattern breaks the expected topology and refuses the evaluation.
    let outer_poly = flat_to_poly(&outline);
    let mut hole_polys: Vec<Vec<[Rat<B>; 2]>> = hole_outlines.iter().map(flat_to_poly).collect();
    hole_polys.extend(domain_polys.iter().cloned());
    hole_polys.extend(part.flat_holes.iter().cloned());
    let expected_holes = hole_polys.len();
    let region = match assemble_flat(&outer_poly, &hole_polys) {
        Verdict::Verified(r) => r,
        Verdict::Unresolved(()) => {
            return Verdict::Unresolved(part.clearance.clone());
        }
        Verdict::Refuted(_) => {
            return Verdict::Refuted(PartFault::TopologyMismatch {
                expected_holes,
                faces: 0,
                holes: 0,
            });
        }
    };
    if region.faces.len() != 1 || region.faces[0].holes.len() != expected_holes {
        return Verdict::Refuted(PartFault::TopologyMismatch {
            expected_holes,
            faces: region.faces.len(),
            holes: region.faces.first().map(|f| f.holes.len()).unwrap_or(0),
        });
    }

    // — 9. The report echo. —
    let report = build_report(part, &structure, &holes);
    Verdict::Verified(FlatPattern {
        outline,
        holes: hole_outlines,
        domain_holes: domain_polys,
        flat_holes: part.flat_holes.clone(),
        region,
        eps: eps_all,
        report,
    })
}

/// Certify the resolved structure into the watertight solid (see the module docs): the boundary
/// re-certified at the low-degree STEP profile with dyadic-snapped corners, handed to
/// `brep_trim_solid_regions` as the piecewise inner (lower-µ̂) / outer (upper-µ̂) chains, the
/// interior holes as [`HoleRail`]s, and the domain-authored holes as `(σ, µ̂)` polygon cuts.
pub(crate) fn solid_brep<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: Structure<B>,
) -> Verdict<SolidParts<B>, PartFault, Rat<B>> {
    // **The region bands, clipped to the derived σ-extent (AUTH.3c).** The authored band is where
    // the blank was declared; `structure.domain` is where the ops actually left material, and the
    // two differ exactly when a cutter terminates the stock in σ. Every consumer below wants the
    // second: `brep_trim_solid_regions` sweeps these bands to place its σ-stations and to pick the
    // rail piece covering each slice, so handing it the authored band asks it to build over σ the
    // boundary chains do not cover — which it refuses, correctly, by failing to find a piece.
    //
    // Clipping in place rather than dropping keeps the region *index* meaningful: `span_pieces`
    // returns it and `find_piece` matches it against the rail pieces' own `region`. A region the
    // extent excludes clips to an empty interval, which `span_pieces` already skips; only the chart
    // list below drops it, because the builder does place geometry per chart.
    let bands: Vec<Interval<B>> = part
        .regions
        .iter()
        .map(|r| Interval {
            lo: rmax(&r.band.lo, &structure.domain.lo),
            hi: rmin(&r.band.hi, &structure.domain.hi),
        })
        .collect();
    // — 3″. The contour IS the boundary, in the solid too. —
    //
    // The flat path takes this fork already (`sole_pinched_contour`): when the part is exactly what
    // one quadric wall keeps, the boundary is that wall's own traced loop and no chain of graph
    // rails can reach its σ-ends. The solid takes it for the same reason and answers it the same
    // way — the loop as a general `(σ,µ̂)` outer wire — so it never calls `certify_boundary`, which
    // would refuse with `RailSpanShort` before a single face was built.
    if let Some(op) = sole_contour(part, built, &structure) {
        return outline_solid(part, built, structure, op);
    }
    // The STEP re-fit: a curved rail exported to OCCT must stay a handful of Bézier control
    // points (the G7 finding) — internal, not a facade knob (seam #8).
    let fit = RailFit {
        subdiv: part.fit.subdiv.max(RailFit::occt_low().subdiv),
        ..RailFit::occt_low()
    };
    let boundary = bail!(certify_boundary(part, built, &structure, fit, true, true));
    let mut eps_all = boundary.eps.clone();

    // The chains: per-side segments split at region joins, as ordered (band, rail) pieces. A
    // flank crossing's splice contributes its **traced route** — the same tails-and-connector the
    // flat pattern draws — chorded into contiguous affine pieces ([`splice_chain`]). It used to
    // contribute one certified chord, and that was #305: the splices had grown from µm-fine tails
    // into whole caps and domes, and hulling each one printed a lug with sides 15° off their
    // rulings and a flat top where the drawing has a dome.
    let chain =
        |segs: &[(Rat<B>, Rat<B>, Label)], splices: &[Splice<B>]| -> Result<Chain<B>, PartFault> {
            use core::cmp::Ordering;
            let mut out = Vec::new();
            for (a, b, label) in segs {
                for (plo, phi, ri) in span_pieces(&bands, a, b) {
                    let piece =
                        find_piece(&boundary.pieces, *label, ri, &plo).ok_or(PartFault::Pole)?;
                    out.push((Interval { lo: plo, hi: phi }, piece.mu.clone()));
                }
                if let Some(sp) = splices.iter().find(|s| s.edge_a.cmp(b) == Ordering::Equal) {
                    splice_chain(sp, &mut out)?;
                }
            }
            Ok(out)
        };
    let outer = match chain(&boundary.upper_segs, &boundary.upper_splices) {
        Ok(c) => c,
        Err(f) => return Verdict::Refuted(f),
    };
    let inner = match chain(&boundary.lower_segs, &boundary.lower_splices) {
        Ok(c) => c,
        Err(f) => return Verdict::Refuted(f),
    };

    // — 3‴. The **mixed** boundary: a rail out and an arc back. —
    //
    // The whole-side splice leaves one chain empty and a two-turn arc in its place (§12.4), so there
    // is no band to sweep — but there *is* still a real rail on the other side, and chording it
    // would trade a certified fit for an unbounded sagitta. The outer wire says both things at once:
    // the arc's chords as explicit points, the rail named as a rail. Only the lower chain can be the
    // empty one, because only case (i) of the splice clears a chain and it clears the lower.
    if inner.is_empty()
        && let Some(arc) = boundary.end_arcs[1]
            .as_ref()
            .or(boundary.end_arcs[0].as_ref())
        && !outer.is_empty()
    {
        // The rail run, named rather than sampled — the whole point of the rail-borne vertex. Its
        // two σ *are* the junctions: the splice leaves exactly the segments between them.
        let mut wire: Vec<WirePoint<B>> = vec![
            WirePoint::OnOuter(outer[0].0.lo.clone()),
            WirePoint::OnOuter(outer[outer.len() - 1].0.hi.clone()),
        ];
        // …then the arc back — each piece chorded at the solid's sagitta budget (one chord per
        // piece was #304's ziggurat) — **skipping its first point**: that one sits on the junction
        // the rail vertex above already carries, and to within the ε-wide ruling gap it is the
        // same point. Keeping both would put a sub-ε radial edge in the wall (the shape #267
        // refused in STEP), and asking `inside_band` whether a junction point is strictly inside
        // the rail it lies on is a question with no right answer.
        let mut arc_pts: Vec<(Rat<B>, Rat<B>)> = Vec::new();
        for piece in arc.iter() {
            if chord_pcurve(piece, &mut arc_pts).is_none() {
                return Verdict::Refuted(PartFault::Pole);
            }
        }
        for (sg, m) in arc_pts.into_iter().skip(1) {
            wire.push(WirePoint::At(snap30(&sg), snap30(&m)));
        }
        let eps = eps_all.clone();
        return wire_solid(part, built, structure, wire, Some(outer), eps);
    }

    let (holes, poly_holes, hole_loops, hole_eps) = match solid_holes(part, built, &structure) {
        Verdict::Verified(h) => h,
        Verdict::Unresolved(e) => return Verdict::Unresolved(e),
        Verdict::Refuted(f) => return Verdict::Refuted(f),
    };
    eps_all = rmax(&eps_all, &hole_eps);

    // Only the regions the extent actually reaches carry geometry. A region clipped to nothing is
    // not a degenerate slab to be built at zero width — it is outside the part.
    let charts: Vec<(Interval<B>, &geom::chart::Chart<B>)> = bands
        .iter()
        .cloned()
        .zip(built.charts.iter())
        .filter(|(iv, _)| iv.lo.cmp(&iv.hi) == core::cmp::Ordering::Less)
        .collect();
    if charts.is_empty() {
        return Verdict::Refuted(PartFault::EmptyRegion);
    }
    let w = match part.thickness_window() {
        Some(w) => w,
        None => return Verdict::Refuted(PartFault::NeutralOutsideStack),
    };
    let solid =
        match brep_trim_solid_regions(&charts, &w, &inner, &outer, None, &holes, &poly_holes) {
            Some(s) => s,
            None => return Verdict::Refuted(PartFault::SolidRefused),
        };
    let report = build_report(part, &structure, &hole_loops);
    Verdict::Verified((solid, eps_all, report))
}

/// Every interior cut of the solid, in the two currencies the builder takes, plus the certified
/// bound over all of them. Shared by both solid evaluators: what bounds a panel from **outside**
/// is what AUTH.3c changed, and holes are the same holes either way.
///
/// Interior holes are p-curve loops (they pass through their tangent rulings rather than being two
/// graphs bridged by a chord). The builder still consumes them as a near/far band — which they are:
/// the branches are functions of σ, just not polynomials near the tangents — so `hole_rail` splits
/// each loop at its two σ-extremes into contiguous rail chains, and the hole may still span
/// σ-stations.
///
/// Fewer hole segments than the flat pattern uses: every piece boundary of a hole's chains becomes a
/// σ-station, so segment count drives the solid's face count directly (48 segments cost ~770 faces
/// on the doctest panel, 16 cost ~250). The solid is already emitted at the low-degree STEP profile,
/// so it takes the coarser loop; the flat pattern — the artifact that is actually manufactured —
/// keeps the fine one. The coarse trace is a *station* economy, not a shape one: each curved piece
/// is chorded against the sagitta budget at conversion (`export::trim::chord_pcurve`, #304), so a
/// cap still reads as its curve while its stations stay few.
#[allow(clippy::type_complexity)]
fn solid_holes<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: &Structure<B>,
) -> Verdict<
    (
        Vec<HoleRail<B>>,
        Vec<Vec<(Rat<B>, Rat<B>)>>,
        Vec<CertifiedHole<B>>,
        Rat<B>,
    ),
    PartFault,
    Rat<B>,
> {
    let mut eps = Rat::from_i128(0);
    let hole_loops = bail!(certify_holes(
        part,
        built,
        structure,
        part.segments.clamp(8, 16)
    ));
    let mut holes: Vec<HoleRail<B>> = Vec::new();
    let mut traced_polys: Vec<Vec<(Rat<B>, Rat<B>)>> = Vec::new();
    for h in &hole_loops {
        let h = &h.boundary;
        eps = rmax(&eps, &h.eps);
        match hole_rail(h) {
            Some(r) => holes.push(r),
            // A loop that turns around in σ more than twice has no near/far split — the shape
            // AUTH.2c's tracer emits for a non-convex footprint. It goes to the builder's general
            // channel instead, as the `(σ, µ̂)` polygon it already is: a lid inner wire plus a wall
            // per edge. The band channel keeps the loops it can carry, because only it spans
            // σ-stations (AUTH.2e lifts that).
            None => match hole_poly(h) {
                Some(p) => traced_polys.push(p),
                None => return Verdict::Refuted(PartFault::LoopBroken),
            },
        }
    }

    // Flat-authored holes: fold each vertex back to `(σ, µ̂)` (the certified piecewise fold, the
    // µ̂-side derived from the resolution), snap to the STEP dyadic grid, and drill them as
    // polygon cuts alongside the domain-authored ones. `fold_point_pw` gates each vertex by the
    // round-trip DRC, so a loose fold surfaces as `Unresolved`, never as a silently drifted hole.
    let mut poly_holes = part.domain_holes.clone();
    poly_holes.extend(traced_polys);
    // A polygon cut needs at least a triangle — the builder indexes vertices unchecked, and
    // this evaluator must stay fail-closed even without the flat gate (defense in depth for
    // both authored hole classes).
    if poly_holes.iter().any(|p| p.len() < 3) {
        return Verdict::Refuted(PartFault::EmptyFeature);
    }
    if !part.flat_holes.is_empty() {
        let side = match structure.mu_negative {
            Some(s) => s,
            None => return Verdict::Refuted(PartFault::SideAmbiguous),
        };
        let zero = Rat::from_i128(0);
        for poly in &part.flat_holes {
            if poly.len() < 3 {
                return Verdict::Refuted(PartFault::EmptyFeature);
            }
            let mut folded: Vec<(Rat<B>, Rat<B>)> = Vec::with_capacity(poly.len());
            for p in poly {
                match develop::fold::fold_point_pw(
                    &built.pw,
                    &built.charts,
                    &p[0],
                    &p[1],
                    &zero,
                    crate::part::FOLD_ITERS,
                    side,
                    &part.cfg,
                    &part.clearance,
                ) {
                    Verdict::Verified(f) => {
                        eps = rmax(&eps, &f.eps);
                        folded.push((snap30(&f.sigma.mid()), snap30(&f.mu.mid())));
                    }
                    Verdict::Unresolved(e) => return Verdict::Unresolved(e),
                    Verdict::Refuted(fault) => {
                        return Verdict::Refuted(crate::part::map_fold_fault(fault));
                    }
                }
            }
            poly_holes.push(folded);
        }
    }
    Verdict::Verified((holes, poly_holes, hole_loops, eps))
}

/// The solid of a part whose boundary **is** its contour's traced loop — [`sole_contour`]'s
/// shape, in the solid (AUTH.3c, `docs/cutter-extrude-design.md` §12.4).
///
/// Nothing here is a special case of the geometry; it is a change of *currency*. The loop's two
/// σ-ends are tangent rulings where the wall's branches meet with unbounded slope, so no chain of
/// fitted graph rails reaches them — `certify_boundary` refuses with `RailSpanShort`, which is
/// correct and is why this fork happens before it. The same loop given as a general `(σ,µ̂)` polygon
/// is unremarkable: the builder's outer-wire channel intersects each slice's strip with it, exactly
/// as the polygon-hole channel subtracts one.
///
/// The band the wire needs is **synthesized** rather than derived, because there is no boundary rail
/// to derive it from. It only has to *contain* the wire: it still fixes the σ-station partition and
/// the ruled patch each footprint is trimmed out of, and both are insensitive to how wide it is.
/// The pad is relative (a sixteenth of the wire's own µ̂-span each side) so the band stays as close
/// to the material as the wire is — a fixed pad on a small contour could reach the chart's singular
/// rail, where the parametrization, not the part, breaks down.
///
/// The certified bound is the traced loop's own `eps`, which is what the flat path reports for the
/// same construction: the wire is that loop's vertices, and its chords are secants of a curve
/// already certified to within it.
fn outline_solid<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: Structure<B>,
    op: usize,
) -> Verdict<SolidParts<B>, PartFault, Rat<B>> {
    // **The outer wire does not take the hole budget.** `certify_holes` clamps to `8..=16` because a
    // hole's segment count drives the solid's face count and a hole is a *feature* — a coarser loop
    // is a fidelity trade. The wire is the **boundary**, so under-resolving it is not a trade: the
    // loop simply fails to certify against the clearance and the part is refused (measured — a
    // radiused outline that certifies flat at 48 segments is `Unresolved` at 16). It takes the
    // part's own resolution, and the face count follows from it.
    let loop_ = match contour_outline(part, built, op, part.segments) {
        Ok(l) => l,
        Err(RErr::Fault(f)) => return Verdict::Refuted(f),
        Err(RErr::Loose(e)) => return Verdict::Unresolved(e),
    };
    let pts = match hole_poly(&loop_) {
        Some(p) if p.len() >= 3 => p,
        _ => return Verdict::Refuted(PartFault::LoopBroken),
    };
    let wire: Vec<WirePoint<B>> = pts.into_iter().map(|(s, m)| WirePoint::At(s, m)).collect();
    let eps = loop_.eps.clone();
    wire_solid(part, built, structure, wire, None, eps)
}

/// Build the solid whose outer boundary is `wire`, over a **synthesized** enclosing band.
///
/// Shared by both outer-wire shapes, because the band is the same idea in each: it does not bound
/// the part, it only has to *contain* it. What it still does is fix the σ-station partition and the
/// ruled patch each footprint is trimmed out of, and neither is sensitive to how wide it is. The
/// pad is relative (a sixteenth of the wire's own µ̂-span each side) so the band stays as close to
/// the material as the wire is — a fixed pad on a small contour could reach the chart's singular
/// rail, where the parametrization breaks down rather than the part.
///
/// `rail`, when given, is the **real** upper rail over its own σ-span, spliced into the synthesized
/// one: the mixed shape's wire runs *along* that rail, and naming it rather than chording it is what
/// keeps that stretch exact and its certified bound the rail fit's rather than a chord sagitta
/// nobody bounded.
fn wire_solid<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: Structure<B>,
    wire: Vec<WirePoint<B>>,
    rail: Option<Chain<B>>,
    wire_eps: Rat<B>,
) -> Verdict<SolidParts<B>, PartFault, Rat<B>> {
    let free: Vec<(Rat<B>, Rat<B>)> = wire
        .iter()
        .filter_map(|p| match p {
            WirePoint::At(s, m) => Some((s.clone(), m.clone())),
            _ => None,
        })
        .collect();
    if free.is_empty() {
        return Verdict::Refuted(PartFault::LoopBroken);
    }
    let (mut s_lo, mut s_hi) = (wire[0].sigma().clone(), wire[0].sigma().clone());
    let (mut m_lo, mut m_hi) = (free[0].1.clone(), free[0].1.clone());
    for p in &wire {
        s_lo = rmin(&s_lo, p.sigma());
        s_hi = rmax(&s_hi, p.sigma());
    }
    for (_, m) in &free {
        m_lo = rmin(&m_lo, m);
        m_hi = rmax(&m_hi, m);
    }
    let pad = m_hi.sub(&m_lo).mul(&Rat::new(1, 16));
    if pad.sign() <= 0 || s_lo.cmp(&s_hi) != core::cmp::Ordering::Less {
        return Verdict::Refuted(PartFault::EmptyRegion);
    }
    let band = Interval { lo: s_lo, hi: s_hi };
    let cst = |v: &Rat<B>| lattice::RatFunc::from_poly(lattice::Poly::constant(v.clone()));
    let inner: Chain<B> = vec![(band.clone(), cst(&m_lo.sub(&pad)))];
    // The upper rail is synthesized *around* whatever real rail the wire runs along: the real one
    // where the wire names it, a constant clear of the material either side. The gaps have to be
    // filled rather than left short because the builder picks one rail piece per slice — and the
    // real rail's own two ends become σ-stations (every chain-piece boundary does), so no slice ever
    // straddles the handover and reads the wrong one.
    let above = cst(&m_hi.add(&pad));
    let mut outer: Chain<B> = Vec::new();
    match rail {
        Some(r) if !r.is_empty() => {
            let (a, b) = (r[0].0.lo.clone(), r[r.len() - 1].0.hi.clone());
            if band.lo.cmp(&a) == core::cmp::Ordering::Less {
                outer.push((
                    Interval {
                        lo: band.lo.clone(),
                        hi: a,
                    },
                    above.clone(),
                ));
            }
            outer.extend(r);
            if b.cmp(&band.hi) == core::cmp::Ordering::Less {
                outer.push((
                    Interval {
                        lo: b,
                        hi: band.hi.clone(),
                    },
                    above,
                ));
            }
        }
        _ => outer.push((band.clone(), above)),
    }

    let (holes, poly_holes, hole_loops, hole_eps) = match solid_holes(part, built, &structure) {
        Verdict::Verified(h) => h,
        Verdict::Unresolved(e) => return Verdict::Unresolved(e),
        Verdict::Refuted(f) => return Verdict::Refuted(f),
    };
    let eps_all = rmax(&wire_eps, &hole_eps);

    let charts: Vec<(Interval<B>, &geom::chart::Chart<B>)> = vec![(band, &built.charts[0])];
    let w = match part.thickness_window() {
        Some(w) => w,
        None => return Verdict::Refuted(PartFault::NeutralOutsideStack),
    };
    let solid = match brep_trim_solid_regions(
        &charts,
        &w,
        &inner,
        &outer,
        Some(&wire),
        &holes,
        &poly_holes,
    ) {
        Some(s) => s,
        None => return Verdict::Refuted(PartFault::SolidRefused),
    };
    let report = build_report(part, &structure, &hole_loops);
    Verdict::Verified((solid, eps_all, report))
}

/// The report echo: snapped region bands, derived op roles, and each hole op's own certified cut
/// bound (the largest over its loops — see [`OpReport::cut_eps`]).
fn build_report<B: Backend>(
    part: &Part<B>,
    structure: &Structure<B>,
    holes: &[CertifiedHole<B>],
) -> ResolveReport<B> {
    let per_op = |op: usize| -> (Option<Rat<B>>, Option<Rat<B>>) {
        holes
            .iter()
            .filter(|h| h.op == op)
            .fold((None, None), |(e, g), h| {
                let up = |acc: Option<Rat<B>>, v: &Rat<B>| {
                    Some(match acc {
                        Some(a) => rmax(&a, v),
                        None => v.clone(),
                    })
                };
                (up(e, &h.boundary.eps), up(g, &h.boundary.tangent_gap))
            })
    };
    ResolveReport {
        regions: part
            .regions
            .iter()
            .map(|r| RegionEcho {
                requested_deg: r.requested_deg,
                band: r.band.clone(),
            })
            .collect(),
        ops: structure
            .roles
            .iter()
            .zip(part.ops.iter())
            .enumerate()
            .map(|(op, (role, (kind, _)))| {
                let (cut_eps, tangent_gap) = per_op(op);
                OpReport {
                    subtract: matches!(kind, crate::part::OpKind::Subtract),
                    role: *role,
                    cut_eps,
                    tangent_gap,
                }
            })
            .collect(),
    }
}

/// The `(σ, µ̂)` endpoint of the last arc pushed so far.
fn last_end<B: Backend>(arcs: &[BoundaryArc<B>]) -> Option<(Rat<B>, Rat<B>)> {
    arcs.last().and_then(|arc| match arc {
        BoundaryArc::Rail { mu, sigma_end, .. } => Some((
            sigma_end.clone(),
            mu.eval(sigma_end).expect("rail evaluable at its own end"),
        )),
        BoundaryArc::Cap { sigma, mu_end, .. } => Some((sigma.clone(), mu_end.clone())),
        // The boundary chain this walks is built from rails and caps; a p-curve arc appears only
        // in an interior cut loop, which is assembled whole rather than chained corner by corner.
        BoundaryArc::Curve { curve, .. } => curve.eval(&curve.domain.hi).map(|[s, m]| (s, m)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::construct;
    use crate::part::{Cutter, SupportFn};
    use develop::cone::DevConfig;
    use export::trim::RailFit;
    use fixtures::devices::{cone, cone_wrap};
    use lattice::{Bignum, Rat};

    type Q = Rat<Bignum>;
    fn q(n: i128, d: i128) -> Q {
        Q::new(n, d)
    }
    fn qi(n: i128) -> Q {
        Q::from_i128(n)
    }

    /// `acceptance::flex_panel()`, copied verbatim — `acceptance` depends on `author`, so an
    /// in-crate test cannot use it without linking a second copy of this crate.
    fn flex_panel() -> Part<Bignum> {
        construct::from_chart::<Bignum>(&cone())
            .region_sigma(qi(-1), qi(1), SupportFn::inherit())
            .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
            .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2)))
            .subtract(Cutter::vertical_cylinder(q(-9, 4), q(9, 4), q(9, 16)))
            .subtract(Cutter::vertical_cylinder(qi(0), q(11, 5), q(1, 25)))
            .clearance(qi(1))
    }

    /// `acceptance::self_lapping_cone(segments, support_panels, true)`, copied verbatim — which
    /// means it moves whenever that does. Kept in step with the device's physical dimensioning
    /// (2026-08-17): `Δ = 1/4`, `t = 6/25`, every length on the same 5/3.
    fn self_lapping_cone(segments: usize, support_panels: usize) -> Part<Bignum> {
        let d = q(1, 4);
        let rz0 = cone_wrap()
            .ruling()
            .comp(2)
            .eval(&qi(0))
            .expect("the wrap chart's ruling is regular at σ = 0");
        let mu_w = q(-5, 1).div(&rz0);
        let witness = cone_wrap()
            .surface(&mu_w, &qi(0))
            .eval(&qi(0))
            .expect("the mid-annulus witness point is regular");
        construct::from_chart::<Bignum>(&cone_wrap())
            .region_sigma(q(-5, 4), q(4, 7), SupportFn::constant(qi(0)))
            .region_sigma(q(4, 7), qi(1), SupportFn::smoothstep(qi(0), d.clone()))
            .region_sigma(qi(1), q(5, 4), SupportFn::constant(d))
            .keep_near(witness)
            .intersect(Cutter::vertical_cylinder(qi(0), qi(0), q(157, 6)))
            .subtract(Cutter::vertical_cylinder(qi(0), q(5, 6), q(100, 9)))
            .clearance(q(5, 3))
            .thickness(q(6, 25))
            .fit(RailFit {
                degree: 4,
                subdiv: 160,
                bits: 44,
            })
            .segments(segments)
            .support_panels(support_panels)
            .budget(DevConfig {
                terms: 14,
                sqrt_eps: q(1, 1_000_000_000),
            })
            .subtract(Cutter::vertical_cylinder(q(-5, 6), q(9, 2), q(5, 72)))
    }

    /// OPT.2.0 Q1 — stage attribution on the real test payloads.
    ///
    /// The two fixtures are a **γ-controlled pair by design**: `flex_panel`'s apex cone has a
    /// vanishing pedal and develops with `γ ≡ 0` throughout, while `self_lapping_cone` carries a
    /// nonzero flat directrix on its ramp and tail. The difference isolates the quadrature.
    /// Run with `cargo test -p author --lib stage_attribution -- --ignored --nocapture`.
    /// Ignored by default: it develops each fixture twice and takes minutes — the very cost
    /// OPT.2 exists to reduce.
    #[test]
    #[ignore = "profiling harness, minutes long; run explicitly"]
    fn stage_attribution() {
        let cases: [(&str, Part<Bignum>); 2] = [
            ("flex_panel   (gamma=0)", flex_panel()),
            ("self_lapping (gamma!=0)", self_lapping_cone(16, 8)),
        ];
        for (name, part) in cases {
            develop::counters::reset();
            let clock = std::time::Instant::now();
            let _ = part.develop();
            let t_total = clock.elapsed().as_secs_f64();
            std::eprintln!(
                "  [gamma] whole develop: {} cells, {} cut_evals",
                develop::counters::gamma_cells(),
                develop::counters::cut_evals()
            );

            let c = std::time::Instant::now();
            let built = part.build_regions().expect("regions develop");
            let t_build = c.elapsed().as_secs_f64();

            let c = std::time::Instant::now();
            let structure = crate::resolve::sweep(&part, &built).expect("the sweep resolves");
            let t_sweep = c.elapsed().as_secs_f64();

            develop::counters::reset();
            let c = std::time::Instant::now();
            let _ = certify_boundary(&part, &built, &structure, part.fit, false, true);
            let t_bnd = c.elapsed().as_secs_f64();
            let (g_b, e_b) = (
                develop::counters::gamma_cells(),
                develop::counters::cut_evals(),
            );

            develop::counters::reset();
            let c = std::time::Instant::now();
            let _ = certify_holes(&part, &built, &structure, (part.segments / 2).max(4));
            let t_holes = c.elapsed().as_secs_f64();
            let (g_h, e_h) = (
                develop::counters::gamma_cells(),
                develop::counters::cut_evals(),
            );

            let rest = t_total - t_build - t_sweep - t_bnd - t_holes;
            std::eprintln!(
                "\n{name}  total {t_total:7.1}s\n\
                 \x20 build_regions {t_build:7.1}s\n\
                 \x20 sweep         {t_sweep:7.1}s\n\
                 \x20 boundary      {t_bnd:7.1}s   (gamma {g_b}, cut_evals {e_b})\n\
                 \x20 holes         {t_holes:7.1}s   (gamma {g_h}, cut_evals {e_h})\n\
                 \x20 rest          {rest:7.1}s   (unroll + flat boolean + topology)"
            );
        }
    }
}
