//! `realize` — certify the resolved structure into a flat pattern.
//!
//! The resolver ([`crate::resolve`]) says *what* bounds the part where; this module makes it
//! certified geometry: every boundary run's rail is fit + certified against its cutter
//! ([`certified_rail_surface`], per region — supports differ across regions), run corners are
//! refined to where the *fitted* rails meet (exact bisection — clean joins, the micro-cap
//! collapses to the bisection residual), the loop is assembled as the one general
//! [`BoundaryArc`] chain and unrolled through the connected piecewise development (chord
//! certificates across region joins ride the PR-1 anchor frames), interior holes get their own
//! certified loops ([`surface_hole_loop`]), and the exact 2-D boolean stitches the panel —
//! whose topology must **reproduce the resolved structure**, else the whole evaluation is
//! refused ([`PartFault::TopologyMismatch`]): a mis-resolution cannot ship.

use crate::part::{FlatPattern, OpReport, Part, PartFault, RegionEcho, ResolveReport};
use crate::resolve::{BranchSide, Label, Structure};
use certify_core::Verdict;
use develop::part::Development;
use develop::unroll::{BoundaryArc, FlatOutline, UnrollFault, unroll_trim_loop};
use export::cut_oracle::RootPick;
use export::trim::{
    RailFit, assemble_flat, bisect_root, certified_rail_surface, flat_to_poly, surface_hole_loop,
};
use lattice::{Backend, Interval, Rat, RatFunc};

use crate::part::BuiltRegions;

/// A per-op exact σ-extent within one region (the two-tangent clamp), or `None` (no extent).
type Extent<B> = Option<(Rat<B>, Rat<B>)>;

/// One fitted + certified rail piece: a label realized on one region.
struct RailPiece<B: Backend> {
    label: Label,
    region: usize,
    mu: RatFunc<B>,
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

/// Evaluate the rail for `label` at σ, preferring the piece whose region contains σ.
fn rail_at<B: Backend>(
    pieces: &[RailPiece<B>],
    regions: &[Interval<B>],
    label: Label,
    sigma: &Rat<B>,
) -> Option<Rat<B>> {
    use core::cmp::Ordering;
    let in_region = |band: &Interval<B>| {
        band.lo.cmp(sigma) != Ordering::Greater && sigma.cmp(&band.hi) != Ordering::Greater
    };
    let exact = pieces
        .iter()
        .find(|p| p.label == label && in_region(&regions[p.region]));
    let piece = exact.or_else(|| pieces.iter().find(|p| p.label == label))?;
    piece.mu.eval(sigma)
}

/// The pieces of `label`'s rail within `[a, b]`, split at region joins:
/// `(from, to, region index)` in ascending σ.
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
    let domain_width = domain.hi.sub(&domain.lo);
    let runs = &structure.runs;
    if runs.is_empty() {
        return Verdict::Refuted(PartFault::EmptyRegion);
    }
    let mut eps_all = Rat::from_i128(0);

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
    // A quadratic cutter's branch is only real between its two tangent rulings — fitting past
    // them makes the oracle decline — so each label's span clamps to the cutter's exact extent
    // (inset a hair, the hole-loop margin doctrine). Planes have no extent.
    let mut extents: Vec<Vec<Extent<B>>> = Vec::new();
    for (ri, band) in bands.iter().enumerate() {
        let mut row = Vec::with_capacity(part.ops.len());
        for (_, cutter) in &part.ops {
            row.push(match cutter {
                crate::part::Cutter::Cylinder { .. } => export::trim::surface_tangents(
                    &built.charts[ri],
                    &cutter.surface(),
                    band,
                    256,
                    60,
                )
                .map(|(t1, t2)| {
                    let inset = t2.sub(&t1).mul(&Rat::new(1, 200));
                    (t1.add(&inset), t2.sub(&inset))
                }),
                crate::part::Cutter::HalfSpace { .. } => None,
            });
        }
        extents.push(row);
    }
    let mut pieces: Vec<RailPiece<B>> = Vec::new();
    for &label in &labels {
        let (lo, hi) = hull_of(label);
        for (span_lo, span_hi, ri) in span_pieces(&bands, &lo, &hi) {
            // Pad by one resolver cell (clamped to the region) so refined corners stay inside
            // the certified span.
            let pad = bands[ri]
                .hi
                .sub(&bands[ri].lo)
                .mul(&Rat::new(1, crate::resolve::CELLS as i128));
            let mut span = Interval {
                lo: rmax(&span_lo.sub(&pad), &bands[ri].lo),
                hi: rmin(&span_hi.add(&pad), &bands[ri].hi),
            };
            if let Some((t1, t2)) = &extents[ri][label.0] {
                span = Interval {
                    lo: rmax(&span.lo, t1),
                    hi: rmin(&span.hi, t2),
                };
            }
            if span.lo.cmp(&span.hi) != Ordering::Less {
                return Verdict::Refuted(PartFault::CutUnresolved { op: label.0 });
            }
            // A narrow off-origin span is ill-conditioned in the monomial basis (the G2/notch
            // finding) — cap the fit degree there.
            let narrow = span
                .hi
                .sub(&span.lo)
                .mul(&Rat::from_i128(4))
                .cmp(&domain_width)
                == Ordering::Less;
            let fit = if narrow && part.fit.degree > 3 {
                RailFit {
                    degree: 3,
                    ..part.fit
                }
            } else {
                part.fit
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
                Verdict::Unresolved(e) => return Verdict::Unresolved(e),
                Verdict::Refuted(_) => {
                    return Verdict::Refuted(PartFault::CutUnresolved { op: label.0 });
                }
            };
            eps_all = rmax(&eps_all, &e);
            pieces.push(RailPiece {
                label,
                region: ri,
                mu,
            });
        }
    }

    // — 3. Refine the run corners on the fitted rails (per changed side). —
    // junctions[i] = the σ where run i hands over to run i+1, per side (upper, lower).
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
            let find = |lab: Label| {
                pieces
                    .iter()
                    .find(|p| p.label == lab && p.region == ri)
                    .or_else(|| pieces.iter().find(|p| p.label == lab))
            };
            match (find(left), find(right)) {
                (Some(l), Some(r)) => {
                    let dmu = l.mu.sub(&r.mu);
                    bisect_root(&dmu, a, b, 60).unwrap_or_else(|| mid.clone())
                }
                _ => mid.clone(),
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

    // — 4. Assemble the one general boundary loop. —
    // Chain segments per side: (from, to, label), from domain.lo to domain.hi.
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

    let eval = |label: Label, sigma: &Rat<B>| rail_at(&pieces, &bands, label, sigma);
    let mut arcs: Vec<BoundaryArc<B>> = Vec::new();
    // The starting cap at σ_lo: lower → upper.
    let (lo0, up0) = match (
        eval(lower_segs[0].2, &domain.lo),
        eval(upper_segs[0].2, &domain.lo),
    ) {
        (Some(a), Some(b)) => (a, b),
        _ => return Verdict::Refuted(PartFault::Pole),
    };
    arcs.push(BoundaryArc::Cap {
        sigma: domain.lo.clone(),
        mu_start: lo0,
        mu_end: up0,
    });
    // Upper chain forward, split at region joins, micro-caps at junctions.
    let push_chain = |arcs: &mut Vec<BoundaryArc<B>>,
                      segs: &[(Rat<B>, Rat<B>, Label)],
                      forward: bool|
     -> Result<(), PartFault> {
        let order: Vec<usize> = if forward {
            (0..segs.len()).collect()
        } else {
            (0..segs.len()).rev().collect()
        };
        for (k, &si) in order.iter().enumerate() {
            let (a, b, label) = &segs[si];
            let mut region_pieces = span_pieces(&bands, a, b);
            if !forward {
                region_pieces.reverse();
            }
            for (plo, phi, ri) in region_pieces {
                let piece = pieces
                    .iter()
                    .find(|p| p.label == *label && p.region == ri)
                    .or_else(|| pieces.iter().find(|p| p.label == *label))
                    .ok_or(PartFault::Pole)?;
                let (start, end) = if forward {
                    (plo.clone(), phi.clone())
                } else {
                    (phi.clone(), plo.clone())
                };
                // Micro-cap joining the previous arc's end to this rail's start value.
                let v = piece.mu.eval(&start).ok_or(PartFault::Pole)?;
                if let Some(prev_end) = last_end(arcs) {
                    if prev_end.1.cmp(&v) != Ordering::Equal {
                        arcs.push(BoundaryArc::Cap {
                            sigma: start.clone(),
                            mu_start: prev_end.1.clone(),
                            mu_end: v,
                        });
                    }
                }
                arcs.push(BoundaryArc::Rail {
                    mu: piece.mu.clone(),
                    sigma_start: start,
                    sigma_end: end,
                    segments: part.segments,
                });
            }
            let _ = k;
        }
        Ok(())
    };
    if let Err(f) = push_chain(&mut arcs, &upper_segs, true) {
        return Verdict::Refuted(f);
    }
    // The far cap at σ_hi: upper → lower.
    let lo1 = match eval(lower_segs[lower_segs.len() - 1].2, &domain.hi) {
        Some(v) => v,
        None => return Verdict::Refuted(PartFault::Pole),
    };
    if let Some(prev_end) = last_end(&arcs) {
        if prev_end.1.cmp(&lo1) != Ordering::Equal {
            arcs.push(BoundaryArc::Cap {
                sigma: domain.hi.clone(),
                mu_start: prev_end.1.clone(),
                mu_end: lo1,
            });
        }
    }
    // Lower chain backward.
    if let Err(f) = push_chain(&mut arcs, &lower_segs, false) {
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
    let mut hole_outlines: Vec<FlatOutline<B>> = Vec::new();
    for &(op, ri) in &structure.holes {
        let loop_v = surface_hole_loop(
            &built.charts[ri],
            &part.ops[op].1.surface(),
            &bands[ri],
            part.fit,
            &part.clearance,
            &part.cfg,
            &Rat::new(1, 200),
            (part.segments / 2).max(4),
        );
        let hole = match loop_v {
            Verdict::Verified(h) => h,
            Verdict::Unresolved(e) => return Verdict::Unresolved(e),
            Verdict::Refuted(develop::cut::CutFitFault::PoleInEval) => {
                return Verdict::Refuted(PartFault::Pole);
            }
            Verdict::Refuted(_) => return Verdict::Refuted(PartFault::CutUnresolved { op }),
        };
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
    let outer_poly = flat_to_poly(&outline);
    let mut hole_polys: Vec<Vec<[Rat<B>; 2]>> = hole_outlines.iter().map(flat_to_poly).collect();
    hole_polys.extend(domain_polys.iter().cloned());
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
    let report = ResolveReport {
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
    };

    Verdict::Verified(FlatPattern {
        outline,
        holes: hole_outlines,
        domain_holes: domain_polys,
        region,
        eps: eps_all,
        report,
    })
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
