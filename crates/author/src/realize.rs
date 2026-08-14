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

use crate::part::{
    BuiltRegions, FlatPattern, OpReport, Part, PartFault, RegionEcho, ResolveReport,
};
use crate::resolve::{BranchSide, Label, Structure};
use certify_core::Verdict;
use develop::part::Development;
use develop::unroll::{BoundaryArc, FlatOutline, UnrollFault, unroll_trim_loop};
use export::brep::Brep;
use export::brep_build::{HoleRail, brep_trim_solid_regions};
use export::cut_oracle::RootPick;
use export::trim::{
    HoleLoop, RailFit, assemble_flat, bisect_root, certified_rail_surface, flat_to_poly, hole_rail,
    surface_hole_loop,
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

/// One fitted + certified rail piece: a label realized on one region.
struct RailPiece<B: Backend> {
    label: Label,
    region: usize,
    mu: RatFunc<B>,
}

/// The certified boundary: the per-region rail pieces, the per-side chain segments
/// `(from, to, label)` covering the domain, and the max rail ε.
struct Boundary<B: Backend> {
    pieces: Vec<RailPiece<B>>,
    upper_segs: Vec<(Rat<B>, Rat<B>, Label)>,
    lower_segs: Vec<(Rat<B>, Rat<B>, Label)>,
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
    let domain = Interval {
        lo: bands[0].lo.clone(),
        hi: bands[bands.len() - 1].hi.clone(),
    };
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
    let mut disc_roots: Vec<Vec<Option<Vec<Rat<B>>>>> = Vec::new();
    for (ri, band) in bands.iter().enumerate() {
        let mut row = Vec::with_capacity(part.ops.len());
        for (_, cutter) in &part.ops {
            row.push(match cutter {
                crate::part::Cutter::Cylinder { .. } => export::trim::surface_disc_roots(
                    &built.charts[ri],
                    &cutter.surface(),
                    band,
                    256,
                    60,
                ),
                crate::part::Cutter::HalfSpace { .. } => None,
            });
        }
        disc_roots.push(row);
    }
    let window_around = |ri: usize, op: usize, at: &Rat<B>| -> Extent<B> {
        let roots = disc_roots[ri][op].as_ref()?;
        for w in roots.windows(2) {
            if w[0].cmp(at) == Ordering::Less && at.cmp(&w[1]) == Ordering::Less {
                let inset = w[1].sub(&w[0]).mul(&Rat::new(1, 200));
                return Some((w[0].add(&inset), w[1].sub(&inset)));
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
            if let Some((t1, t2)) = window_around(ri, label.0, &mid) {
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
            };
            let (mu, e) = match certified_rail_surface(
                &built.charts[ri],
                &part.ops[label.0].1.surface(),
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
    let upper_segs = side_segments(&upper_junctions, &|i| runs[i].upper);
    let lower_segs = side_segments(&lower_junctions, &|i| runs[i].lower);

    Ok(Boundary {
        pieces,
        upper_segs,
        lower_segs,
        eps,
    })
}

/// Certify each hole op's loop (extent, both branch rails, micro-caps) at the given fit.
fn certify_holes<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: &Structure<B>,
    fit: RailFit,
    segments: usize,
) -> Result<Vec<HoleLoop<B>>, RErr<B>> {
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
        let loop_v = surface_hole_loop(
            &built.charts[ri],
            &part.ops[op].1.surface(),
            &span,
            fit,
            &part.clearance,
            &part.cfg,
            &Rat::new(1, 200),
            segments,
        );
        match loop_v {
            Verdict::Verified(h) => out.push(h),
            Verdict::Unresolved(e) => return Err(RErr::Loose(e)),
            Verdict::Refuted(develop::cut::CutFitFault::PoleInEval) => {
                return Err(RErr::Fault(PartFault::Pole));
            }
            Verdict::Refuted(_) => return Err(RErr::Fault(PartFault::CutUnresolved { op })),
        }
    }
    Ok(out)
}

/// Certify the resolved structure into the [`FlatPattern`] (see the module docs).
pub(crate) fn flat_pattern<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    structure: Structure<B>,
) -> Verdict<FlatPattern<B>, PartFault, Rat<B>> {
    use core::cmp::Ordering;
    let bands: Vec<Interval<B>> = part.regions.iter().map(|r| r.band.clone()).collect();
    let domain = Interval {
        lo: bands[0].lo.clone(),
        hi: bands[bands.len() - 1].hi.clone(),
    };
    let boundary = bail!(certify_boundary(part, built, &structure, part.fit, false));
    let mut eps_all = boundary.eps.clone();

    // — 4. Assemble the one general boundary loop. —
    let eval = |label: Label, sigma: &Rat<B>| rail_at(&boundary.pieces, &bands, label, sigma);
    let mut arcs: Vec<BoundaryArc<B>> = Vec::new();
    // The starting cap at σ_lo: lower → upper.
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
    // The far cap at σ_hi: upper → lower.
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
    if let Err(f) = push_chain(&mut arcs, &boundary.lower_segs, false) {
        return Verdict::Refuted(f);
    }

    // — 5. Unroll the boundary through the connected piecewise development. —
    let outline = match unroll_trim_loop(&built.pw, &arcs, &part.cfg, &part.clearance) {
        Verdict::Verified(o) => o,
        Verdict::Unresolved(e) => return Verdict::Unresolved(e),
        Verdict::Refuted(UnrollFault::PoleInEval) => return Verdict::Refuted(PartFault::Pole),
        Verdict::Refuted(_) => return Verdict::Refuted(PartFault::LoopBroken),
    };
    eps_all = rmax(&eps_all, &outline.eps);

    // — 6. Interior holes: certified loops + unroll. —
    let holes = bail!(certify_holes(
        part,
        built,
        &structure,
        part.fit,
        (part.segments / 2).max(4),
    ));
    let mut hole_outlines: Vec<FlatOutline<B>> = Vec::new();
    for hole in &holes {
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
    let report = build_report(part, &structure);
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
    let bands: Vec<Interval<B>> = part.regions.iter().map(|r| r.band.clone()).collect();
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

    // Interior holes as near/far HoleRails (the tangent σ dyadic-snapped inside `hole_rail`).
    let hole_loops = bail!(certify_holes(part, built, &structure, fit, 4));
    let mut holes: Vec<HoleRail<B>> = Vec::new();
    for h in &hole_loops {
        eps_all = rmax(&eps_all, &h.eps);
        match hole_rail(h) {
            Some(r) => holes.push(r),
            None => return Verdict::Refuted(PartFault::LoopBroken),
        }
    }

    // Flat-authored holes: fold each vertex back to `(σ, µ̂)` (the certified piecewise fold, the
    // µ̂-side derived from the resolution), snap to the STEP dyadic grid, and drill them as
    // polygon cuts alongside the domain-authored ones. `fold_point_pw` gates each vertex by the
    // round-trip DRC, so a loose fold surfaces as `Unresolved`, never as a silently drifted hole.
    let mut poly_holes = part.domain_holes.clone();
    if !part.flat_holes.is_empty() {
        let side = match structure.mu_negative {
            Some(s) => s,
            None => return Verdict::Refuted(PartFault::SideAmbiguous),
        };
        let zero = Rat::from_i128(0);
        for poly in &part.flat_holes {
            if poly.is_empty() {
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
                        eps_all = rmax(&eps_all, &f.eps);
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

    let charts: Vec<(Interval<B>, &geom::chart::Chart<B>)> =
        bands.iter().cloned().zip(built.charts.iter()).collect();
    let w = Interval {
        lo: Rat::from_i128(0),
        hi: part.thickness.clone(),
    };
    let solid = match brep_trim_solid_regions(&charts, &w, &inner, &outer, &holes, &poly_holes) {
        Some(s) => s,
        None => return Verdict::Refuted(PartFault::SolidRefused),
    };
    let report = build_report(part, &structure);
    Verdict::Verified((solid, eps_all, report))
}

/// The report echo: snapped region bands + derived op roles.
fn build_report<B: Backend>(part: &Part<B>, structure: &Structure<B>) -> ResolveReport<B> {
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
            .map(|(role, (kind, _))| OpReport {
                subtract: matches!(kind, crate::part::OpKind::Subtract),
                role: *role,
            })
            .collect(),
    }
}

/// The `(σ, µ̂)` endpoint of the last arc pushed so far.
fn last_end<B: Backend>(arcs: &[BoundaryArc<B>]) -> Option<(Rat<B>, Rat<B>)> {
    arcs.last().map(|arc| match arc {
        BoundaryArc::Rail { mu, sigma_end, .. } => (
            sigma_end.clone(),
            mu.eval(sigma_end).expect("rail evaluable at its own end"),
        ),
        BoundaryArc::Cap { sigma, mu_end, .. } => (sigma.clone(), mu_end.clone()),
    })
}
