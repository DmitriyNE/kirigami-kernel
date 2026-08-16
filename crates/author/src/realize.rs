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
use develop::part::Development;
use develop::unroll::{BoundaryArc, FlatOutline, UnrollFault, unroll_trim_loop};
use export::brep::Brep;
use export::brep_build::{HoleRail, brep_trim_solid_regions};
use export::cut_oracle::RootPick;
use export::trim::{
    HoleLoop, RailFit, assemble_flat, bisect_root, certified_rail_surface, flat_to_poly, hole_poly,
    hole_rail, shadow_hole_loops, surface_hole_loop,
};
use lattice::{Backend, Interval, Rat, RatFunc};

/// A per-op exact σ-extent within one region (the two-tangent clamp), or `None` (no extent).
type Extent<B> = Option<(Rat<B>, Rat<B>)>;

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
            Err(RErr::Loose(e)) => return Verdict::Unresolved(e),
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

/// The certified boundary: the per-region rail pieces, the per-side chain segments
/// `(from, to, label)` covering the domain, and the max rail ε.
struct Boundary<B: Backend> {
    pieces: Vec<RailPiece<B>>,
    upper_segs: Vec<(Rat<B>, Rat<B>, Label)>,
    lower_segs: Vec<(Rat<B>, Rat<B>, Label)>,
    /// The turn arc closing each σ-end (`[lower end, upper end]`), where that end is a **smooth
    /// pinch** — a tangent ruling of one quadric wall, which no graph rail reaches. Where it is
    /// `None` the end closes with a ruling cap, as it always did. When an arc is present the
    /// outermost segment on *both* chains has been removed: the arc replaces
    /// `[upper rail tail] + [cap] + [lower rail head]`, joining the graph rails at the two
    /// junctions the run-corner refinement located.
    end_arcs: [Option<Vec<develop::pcurve::PCurve<B>>>; 2],
    eps: Rat<B>,
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

/// Find `label`'s rail piece preferring `region`, falling back to any region.
fn find_piece<B: Backend>(
    pieces: &[RailPiece<B>],
    label: Label,
    region: usize,
) -> Option<&RailPiece<B>> {
    pieces
        .iter()
        .find(|p| p.label == label && p.region == region)
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
    find_piece(pieces, label, ri)?.mu.eval(sigma)
}

/// Snap a σ to the `2⁻³⁰` dyadic grid (the STEP corner discipline — huge-denominator corner σ
/// make exported Bézier control points drift off OCCT's `f64` vertices).
fn snap30<B: Backend>(x: &Rat<B>) -> Rat<B> {
    export::approx::f64_to_rat::<B>(export::approx::rat_to_f64(x), 30)
}

/// Steps 1–3 of both evaluators: fit + certify every boundary label's rail per region (spans
/// clamped to the cutter's exact two-tangent extent), refine the run corners on the fitted
/// rails, and fold the runs into per-side chain segments covering the domain. `snap_corners`
/// applies the STEP dyadic snap to the refined junctions.
fn certify_boundary<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: &Structure<B>,
    fit_base: RailFit,
    snap_corners: bool,
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
    let hull_of = |label: Label| -> (Rat<B>, Rat<B>) {
        let (mut lo, mut hi): (Option<Rat<B>>, Option<Rat<B>>) = (None, None);
        for (i, run) in runs.iter().enumerate() {
            if run.lower != label && run.upper != label {
                continue;
            }
            // Extend into the event brackets (the true corner lies between the samples), and to
            // the domain ends on the outermost runs.
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
            lo = Some(match lo {
                None => a.clone(),
                Some(x) => rmin(&x, &a),
            });
            hi = Some(match hi {
                None => b.clone(),
                Some(x) => rmax(&x, &b),
            });
        }
        (lo.expect("label appears in a run"), hi.expect("label"))
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
    /// Per region, per op, per wall: the brackets isolating the wall's tangent rulings, or `None`
    /// where the wall is affine and has no windows.
    type DiscRoots<B> = Vec<Vec<Vec<Option<Vec<Interval<B>>>>>>;
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
                        develop::cut::tangent_events(&f, band, &crate::resolve::tangent_tol()).ok()
                    }
                    _ => None,
                });
            }
            row.push(per_wall);
        }
        disc_roots.push(row);
    }
    let window_around = |ri: usize, op: usize, wall: usize, at: &Rat<B>| -> Extent<B> {
        let brackets = disc_roots[ri][op].get(wall)?.as_ref()?;
        for w in brackets.windows(2) {
            let (lo, hi) = (&w[0].hi, &w[1].lo);
            if lo.cmp(at) == Ordering::Less && at.cmp(hi) == Ordering::Less {
                let inset = hi.sub(lo).mul(&Rat::new(1, 200));
                return Some((lo.add(&inset), hi.sub(&inset)));
            }
        }
        None
    };
    let mut pieces: Vec<RailPiece<B>> = Vec::new();
    for &label in &labels {
        let (lo, hi) = hull_of(label);
        for (span_lo, span_hi, ri) in span_pieces(&bands, &lo, &hi) {
            // The hull already extends into the event brackets, so every refined corner lies
            // inside the certified span; no further padding (over-reach walks the fit into the
            // cutter's √-branch endpoints, where the oracle rightly declines).
            let mut span = Interval {
                lo: span_lo,
                hi: span_hi,
            };
            let mid = span.lo.add(&span.hi).mul(&Rat::new(1, 2));
            if let Some((t1, t2)) = window_around(ri, label.0, crate::resolve::wall_of(label), &mid)
            {
                span = Interval {
                    lo: rmax(&span.lo, &t1),
                    hi: rmin(&span.hi, &t2),
                };
            }
            if span.lo.cmp(&span.hi) != Ordering::Less {
                return Err(RErr::Fault(PartFault::CutUnresolved { op: label.0 }));
            }
            // A narrow off-origin span is ill-conditioned in the monomial basis (the G2/notch
            // finding) — cap the fit degree there.
            let narrow = span
                .hi
                .sub(&span.lo)
                .mul(&Rat::from_i128(4))
                .cmp(&domain_width)
                == Ordering::Less;
            let fit = if narrow && fit_base.degree > 3 {
                RailFit {
                    degree: 3,
                    ..fit_base
                }
            } else {
                fit_base
            };
            let pick = match label.1 {
                BranchSide::Lower => RootPick::Lower,
                BranchSide::Upper | BranchSide::Plane => RootPick::Upper,
                BranchSide::Wall(_, upper) => {
                    if upper {
                        RootPick::Upper
                    } else {
                        RootPick::Lower
                    }
                }
            };
            let walls = part.ops[label.0]
                .1
                .walls()
                .map_err(|_| RErr::Fault(PartFault::CutUnresolved { op: label.0 }))?;
            let (mu, e) = match certified_rail_surface(
                &built.charts[ri],
                &walls[crate::resolve::wall_of(label)],
                pick,
                &span,
                fit,
                &part.clearance,
                &part.cfg,
            ) {
                Verdict::Verified(x) => x,
                Verdict::Unresolved(e) => return Err(RErr::Loose(e)),
                Verdict::Refuted(_) => {
                    return Err(RErr::Fault(PartFault::CutUnresolved { op: label.0 }));
                }
            };
            eps = rmax(&eps, &e);
            pieces.push(RailPiece {
                label,
                region: ri,
                mu,
                span,
            });
        }
    }

    // — 3. Refine the run corners on the fitted rails (per changed side). —
    let mut upper_junctions: Vec<Rat<B>> = Vec::new();
    let mut lower_junctions: Vec<Rat<B>> = Vec::new();
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
                find_piece(&pieces, left, ri),
                find_piece(&pieces, right, ri),
            ) {
                (Some(l), Some(r)) => {
                    let dmu = l.mu.sub(&r.mu);
                    bisect_root(&dmu, a, b, 60).unwrap_or_else(|| mid.clone())
                }
                _ => mid.clone(),
            };
            if snap_corners {
                snap30(&corner)
            } else {
                corner
            }
        };
        upper_junctions.push(if runs[i].upper != runs[i + 1].upper {
            refine(runs[i].upper, runs[i + 1].upper)
        } else {
            mid.clone()
        });
        lower_junctions.push(if runs[i].lower != runs[i + 1].lower {
            refine(runs[i].lower, runs[i + 1].lower)
        } else {
            mid
        });
    }

    // Fold the runs into per-side chain segments (from, to, label) covering the domain.
    let side_segments = |junctions: &[Rat<B>], label_of: &dyn Fn(usize) -> Label| {
        let mut segs: Vec<(Rat<B>, Rat<B>, Label)> = Vec::new();
        for (i, _) in runs.iter().enumerate() {
            let from = if i == 0 {
                domain.lo.clone()
            } else {
                junctions[i - 1].clone()
            };
            let to = if i + 1 == runs.len() {
                domain.hi.clone()
            } else {
                junctions[i].clone()
            };
            match segs.last_mut() {
                Some(last) if last.2 == label_of(i) => last.1 = to,
                _ => segs.push((from, to, label_of(i))),
            }
        }
        segs
    };
    let mut upper_segs = side_segments(&upper_junctions, &|i| runs[i].upper);
    let mut lower_segs = side_segments(&lower_junctions, &|i| runs[i].lower);

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
                let piece = find_piece(&pieces, *label, ri)
                    .ok_or(RErr::Fault(PartFault::CutUnresolved { op: label.0 }))?;
                if plo.cmp(&piece.span.lo) == Ordering::Less
                    || piece.span.hi.cmp(&phi) == Ordering::Less
                {
                    return Err(RErr::Fault(PartFault::RailSpanShort { op: label.0 }));
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
        end_arcs,
        eps,
    })
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
) -> Result<Vec<(usize, HoleLoop<B>)>, RErr<B>> {
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
            Verdict::Verified(hs) => out.extend(hs.into_iter().map(|h| (op, h))),
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
/// below by the same wall of the same intersect, and both derived σ-ends its own tangent rulings.
///
/// This is the shape a graph chain cannot express and a p-curve can (`docs/cutter-extrude-design.md`
/// §12.4). At a tangent ruling the wall's two branches meet with **unbounded slope**, so
/// `certified_rail_surface` clamps its fit away from it and the chain runs out of certificate
/// (`PartFault::RailSpanShort`). The traced loop has no such trouble: it is parametric, passes
/// *through* the tangent, and is exactly what PC.3 built for interior holes — the same construction,
/// used as an outline rather than as a hole.
///
/// Deliberately narrow, and each condition is load-bearing rather than defensive:
///
/// - **one region**, because a loop spanning a region join would need the anchor frames threaded
///   through the tracer, which is not this slice;
/// - **one wall**, so the loop is [`surface_hole_loop`]'s single-quadric construction. A polygonal
///   contour has several, and needs none of this: its walls are affine, `plane_cut_rail` is exact,
///   and its corners are transverse crossings of two straight rails that the chain assembly already
///   carries at `ε = 0`;
/// - **a genuine quadratic** (`a ≢ 0`), which is what makes the end a *smooth* pinch. An affine
///   wall's `disc = b²` vanishes where the crossing escapes to infinity, not where two branches meet.
fn sole_pinched_contour<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: &Structure<B>,
) -> Option<usize> {
    use crate::part::OpKind;
    use crate::resolve::{SigmaEnd, wall_of};
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
        .all(|r| r.lower.0 == op && r.upper.0 == op && wall_of(r.lower) == wall_of(r.upper))
    {
        return None;
    }
    // Both ends closed inside the band, by a pinch rather than a jump.
    if !structure
        .ends
        .iter()
        .all(|e| matches!(e, SigmaEnd::Closed { pinch: true, .. }))
    {
        return None;
    }
    let walls = part.ops[op].1.walls().ok()?;
    if walls.len() != 1 {
        return None;
    }
    let form = develop::cut::cut_mu_form(&built.charts[0], &walls[0], &Rat::from_i128(0))?;
    if form.a.is_zero() {
        return None;
    }
    Some(op)
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
    if let Some(op) = sole_pinched_contour(part, built, &structure) {
        let walls = match part.ops[op].1.walls() {
            Ok(w) => w,
            Err(_) => return Verdict::Refuted(PartFault::CutUnresolved { op }),
        };
        let hole = match surface_hole_loop(
            &built.charts[0],
            &walls[0],
            &bands[0],
            &part.clearance,
            &part.cfg,
            part.segments,
        ) {
            Verdict::Verified(h) => h,
            Verdict::Unresolved(e) => return Verdict::Unresolved(e),
            Verdict::Refuted(develop::cut::CutFitFault::PoleInEval) => {
                return Verdict::Refuted(PartFault::Pole);
            }
            Verdict::Refuted(_) => return Verdict::Refuted(PartFault::CutUnresolved { op }),
        };
        let outline = match unroll_trim_loop(&built.pw, &hole.arcs, &part.cfg, &part.clearance) {
            Verdict::Verified(o) => o,
            Verdict::Unresolved(e) => return Verdict::Unresolved(e),
            Verdict::Refuted(UnrollFault::PoleInEval) => return Verdict::Refuted(PartFault::Pole),
            Verdict::Refuted(_) => return Verdict::Refuted(PartFault::LoopBroken),
        };
        let eps_all = rmax(&hole.eps, &outline.eps);
        return pattern_from_outline(part, built, structure, outline, eps_all);
    }

    let boundary = bail!(certify_boundary(part, built, &structure, part.fit, false));
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
    // Each chain: rails split at region joins, micro-caps at junctions.
    let push_chain = |arcs: &mut Vec<BoundaryArc<B>>,
                      segs: &[(Rat<B>, Rat<B>, Label)],
                      forward: bool|
     -> Result<(), PartFault> {
        let order: Vec<usize> = if forward {
            (0..segs.len()).collect()
        } else {
            (0..segs.len()).rev().collect()
        };
        for &si in &order {
            let (a, b, label) = &segs[si];
            let mut region_pieces = span_pieces(&bands, a, b);
            if !forward {
                region_pieces.reverse();
            }
            for (plo, phi, ri) in region_pieces {
                let piece = find_piece(&boundary.pieces, *label, ri).ok_or(PartFault::Pole)?;
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
                arcs.push(BoundaryArc::Rail {
                    mu: piece.mu.clone(),
                    sigma_start: start,
                    sigma_end: end,
                    segments: part.segments,
                });
            }
        }
        Ok(())
    };
    if let Err(f) = push_chain(&mut arcs, &boundary.upper_segs, true) {
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
    if let Err(f) = push_chain(&mut arcs, &boundary.lower_segs, false) {
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
        Verdict::Unresolved(e) => return Verdict::Unresolved(e),
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
    for (_, hole) in &holes {
        eps_all = rmax(&eps_all, &hole.eps);
        let flat = match unroll_trim_loop(&built.pw, &hole.arcs, &part.cfg, &part.clearance) {
            Verdict::Verified(o) => o,
            Verdict::Unresolved(e) => return Verdict::Unresolved(e),
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
        Verdict::Unresolved(()) => return Verdict::Unresolved(part.clearance.clone()),
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
    if let Some(op) = sole_pinched_contour(part, built, &structure) {
        return outline_solid(part, built, structure, op);
    }
    // The STEP re-fit: a curved rail exported to OCCT must stay a handful of Bézier control
    // points (the G7 finding) — internal, not a facade knob (seam #8).
    let fit = RailFit {
        subdiv: part.fit.subdiv.max(RailFit::occt_low().subdiv),
        ..RailFit::occt_low()
    };
    let boundary = bail!(certify_boundary(part, built, &structure, fit, true));
    let mut eps_all = boundary.eps.clone();

    // The chains: per-side segments split at region joins, as ordered (band, rail) pieces.
    let chain = |segs: &[(Rat<B>, Rat<B>, Label)]| -> Result<Chain<B>, PartFault> {
        let mut out = Vec::new();
        for (a, b, label) in segs {
            for (plo, phi, ri) in span_pieces(&bands, a, b) {
                let piece = find_piece(&boundary.pieces, *label, ri).ok_or(PartFault::Pole)?;
                out.push((Interval { lo: plo, hi: phi }, piece.mu.clone()));
            }
        }
        Ok(out)
    };
    let outer = match chain(&boundary.upper_segs) {
        Ok(c) => c,
        Err(f) => return Verdict::Refuted(f),
    };
    let inner = match chain(&boundary.lower_segs) {
        Ok(c) => c,
        Err(f) => return Verdict::Refuted(f),
    };

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
    let w = Interval {
        lo: Rat::from_i128(0),
        hi: part.thickness.clone(),
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
/// keeps the fine one.
#[allow(clippy::type_complexity)]
fn solid_holes<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: &Structure<B>,
) -> Verdict<
    (
        Vec<HoleRail<B>>,
        Vec<Vec<(Rat<B>, Rat<B>)>>,
        Vec<(usize, HoleLoop<B>)>,
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
    for (_, h) in &hole_loops {
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

/// The solid of a part whose boundary **is** one traced contour loop — [`sole_pinched_contour`]'s
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
    let walls = match part.ops[op].1.walls() {
        Ok(w) => w,
        Err(_) => return Verdict::Refuted(PartFault::CutUnresolved { op }),
    };
    let loop_ = match surface_hole_loop(
        &built.charts[0],
        &walls[0],
        &part.regions[0].band,
        &part.clearance,
        &part.cfg,
        part.segments.clamp(8, 16),
    ) {
        Verdict::Verified(h) => h,
        Verdict::Unresolved(e) => return Verdict::Unresolved(e),
        Verdict::Refuted(develop::cut::CutFitFault::PoleInEval) => {
            return Verdict::Refuted(PartFault::Pole);
        }
        Verdict::Refuted(_) => return Verdict::Refuted(PartFault::CutUnresolved { op }),
    };
    let wire = match hole_poly(&loop_) {
        Some(p) if p.len() >= 3 => p,
        _ => return Verdict::Refuted(PartFault::LoopBroken),
    };
    let mut eps_all = loop_.eps.clone();

    let (mut s_lo, mut s_hi) = (wire[0].0.clone(), wire[0].0.clone());
    let (mut m_lo, mut m_hi) = (wire[0].1.clone(), wire[0].1.clone());
    for (s, m) in &wire {
        s_lo = rmin(&s_lo, s);
        s_hi = rmax(&s_hi, s);
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
    let outer: Chain<B> = vec![(band.clone(), cst(&m_hi.add(&pad)))];

    let (holes, poly_holes, hole_loops, hole_eps) = match solid_holes(part, built, &structure) {
        Verdict::Verified(h) => h,
        Verdict::Unresolved(e) => return Verdict::Unresolved(e),
        Verdict::Refuted(f) => return Verdict::Refuted(f),
    };
    eps_all = rmax(&eps_all, &hole_eps);

    let charts: Vec<(Interval<B>, &geom::chart::Chart<B>)> = vec![(band, &built.charts[0])];
    let w = Interval {
        lo: Rat::from_i128(0),
        hi: part.thickness.clone(),
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
    holes: &[(usize, HoleLoop<B>)],
) -> ResolveReport<B> {
    let per_op = |op: usize| -> (Option<Rat<B>>, Option<Rat<B>>) {
        holes
            .iter()
            .filter(|(o, _)| *o == op)
            .fold((None, None), |(e, g), (_, h)| {
                let up = |acc: Option<Rat<B>>, v: &Rat<B>| {
                    Some(match acc {
                        Some(a) => rmax(&a, v),
                        None => v.clone(),
                    })
                };
                (up(e, &h.eps), up(g, &h.tangent_gap))
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

    /// `acceptance::self_lapping_cone(segments, support_panels, true)`, copied verbatim.
    fn self_lapping_cone(segments: usize, support_panels: usize) -> Part<Bignum> {
        let d = q(1, 10);
        let rz0 = cone_wrap()
            .ruling()
            .comp(2)
            .eval(&qi(0))
            .expect("the wrap chart's ruling is regular at σ = 0");
        let mu_w = q(-3, 1).div(&rz0);
        let witness = cone_wrap()
            .surface(&mu_w, &qi(0))
            .eval(&qi(0))
            .expect("the mid-annulus witness point is regular");
        construct::from_chart::<Bignum>(&cone_wrap())
            .region_sigma(q(-5, 4), q(1, 2), SupportFn::constant(qi(0)))
            .region_sigma(q(1, 2), qi(1), SupportFn::smoothstep(qi(0), d.clone()))
            .region_sigma(qi(1), q(5, 4), SupportFn::constant(d))
            .keep_near(witness)
            .intersect(Cutter::vertical_cylinder(qi(0), qi(0), q(471, 50)))
            .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(4)))
            .clearance(qi(1))
            .thickness(q(1, 20))
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
            .subtract(Cutter::vertical_cylinder(q(-1, 2), q(27, 10), q(1, 40)))
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
            let _ = certify_boundary(&part, &built, &structure, part.fit, false);
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
