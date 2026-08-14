//! `resolve` — the **in-domain material resolution** (the witness doctrine's pre-pass).
//!
//! Every material op's cutter pulls back to a µ̂-form on each region's chart
//! ([`cut_mu_form`](develop::cut::cut_mu_form) — degree ≤ 2, inside ⇔ negative). At each sample
//! σ the kept material is a finite union of µ̂-intervals computed by interval algebra on the op
//! shadows; the sweep across σ derives the **structure** the realizer certifies: which op/branch
//! bounds the part from below and above on which σ-runs (bounding rails and rim notches), and
//! which subtract ops pierce the interior (holes).
//!
//! **The resolver is float machinery and never decides alone** (the D2 contract): its mechanism
//! is an implementation detail, but its *conclusiveness* is not — an unattributable structure
//! faults [`PartFault::AmbiguousRegion`], never guesses. Everything it derives is re-checked
//! downstream: every boundary rail is fit-certified against its cutter (`cut_fit`), the unroll
//! carries chord certificates, and the exact flat boolean must reproduce the resolved topology
//! ([`PartFault::TopologyMismatch`]) — a mis-resolution is refused, not shipped.
//!
//! Stock discipline: unbounded material components are **not part** (a manufactured part is
//! compact) and are dropped; if *nothing* bounded remains the recipe faults
//! [`PartFault::UnboundedRegion`].

use crate::part::{BuiltRegions, Cutter, OpKind, OpRole, Part, PartFault, RegionPick};
use develop::cut::{MuCut, cut_mu_form};
use export::approx::rat_to_f64;
use export::trim::surface_disc_roots;
use lattice::{Backend, Interval, Rat};

/// Which µ̂-root of an op's pullback a boundary label refers to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BranchSide {
    /// The smaller quadratic root.
    Lower,
    /// The larger quadratic root.
    Upper,
    /// The single root of an affine (plane) pullback.
    Plane,
}

/// A boundary label: which op's which branch bounds the kept material here.
pub(crate) type Label = (usize, BranchSide);

/// A labeled µ̂ endpoint (float position, exact label) — `None` at ±∞.
type End = Option<(f64, Label)>;

/// One maximal σ-run of constant boundary structure.
pub(crate) struct Run<B: Backend> {
    /// The first sample σ of the run.
    pub lo: Rat<B>,
    /// The last sample σ of the run.
    pub hi: Rat<B>,
    /// The op/branch bounding from below throughout the run.
    pub lower: Label,
    /// The op/branch bounding from above throughout the run.
    pub upper: Label,
}

/// The resolved structure the realizer certifies.
pub(crate) struct Structure<B: Backend> {
    /// The boundary runs, in σ order, covering the declared domain.
    pub runs: Vec<Run<B>>,
    /// Ops classified as interior holes: `(op, region, the disc-positive σ-window)`.
    pub holes: Vec<(usize, usize, Interval<B>)>,
    /// The derived role per op.
    pub roles: Vec<OpRole>,
    /// The kept material's µ̂-side, when it is uniform across every sample: `Some(true)` = all
    /// µ̂ < 0, `Some(false)` = all µ̂ > 0, `None` = mixed or 0-straddling. The fold's side
    /// convention (derived, never authored — the seam-#3 doctrine).
    pub mu_negative: Option<bool>,
}

/// The resolver's sample-grid density per region (also the realizer's corner pad unit).
pub(crate) const CELLS: usize = 48;

/// A µ̂-shadow of one op at one σ (float, labels exact).
enum Shadow {
    Empty,
    All,
    /// Inside is `µ̂ ∈ [lo, hi]`.
    Between(f64, f64, Label, Label),
    /// Inside is `µ̂ ≤ r`.
    Below(f64, Label),
    /// Inside is `µ̂ ≥ r`.
    Above(f64, Label),
}

/// One kept-material component at one σ: bounds are `None` at ±∞.
#[derive(Clone, Copy)]
struct Comp {
    lo: End,
    hi: End,
}

/// The per-op pullbacks on one region's chart, plus the chart's **singular rail** — the µ̂ where
/// `det J = 0` at `w = 0` (`µ̂ₛ(σ) = −(c′·n′)/(r′·n′)`, exact rational; the apex line on a cone).
/// Material components on opposite sides of it lie on different sheets of the parametrization,
/// so a gap crossing it is never an interior hole.
pub(crate) struct RegionForms<B: Backend> {
    pub band: Interval<B>,
    pub forms: Vec<MuCut<B>>,
    pub detj_c: lattice::RatFunc<B>,
    pub detj_m: lattice::RatFunc<B>,
}

/// The op's µ̂-shadow at σ, from its pullback coefficients (exact eval, float roots).
fn shadow_at<B: Backend>(form: &MuCut<B>, op: usize, sigma: &Rat<B>) -> Option<Shadow> {
    let a = rat_to_f64(&form.a.eval(sigma)?);
    let b = rat_to_f64(&form.b.eval(sigma)?);
    let c = rat_to_f64(&form.c.eval(sigma)?);
    let tiny = 1e-12 * (1.0 + a.abs().max(b.abs()).max(c.abs()));
    Some(if a.abs() <= tiny {
        if b.abs() <= tiny {
            // A degenerate section (plane through the ruling): all-or-nothing by sign of c.
            if c < 0.0 { Shadow::All } else { Shadow::Empty }
        } else {
            let r = -c / b;
            if b > 0.0 {
                Shadow::Below(r, (op, BranchSide::Plane))
            } else {
                Shadow::Above(r, (op, BranchSide::Plane))
            }
        }
    } else {
        // a > 0 structurally (Cauchy–Schwarz); inside = between the roots.
        let disc = b * b - 4.0 * a * c;
        if disc <= 0.0 {
            Shadow::Empty
        } else {
            let sq = disc.sqrt();
            Shadow::Between(
                (-b - sq) / (2.0 * a),
                (-b + sq) / (2.0 * a),
                (op, BranchSide::Lower),
                (op, BranchSide::Upper),
            )
        }
    })
}

/// Intersect a component with a shadow (0 or 1 result).
fn comp_intersect(k: &Comp, sh: &Shadow) -> Vec<Comp> {
    let (slo, shi): (End, End) = match sh {
        Shadow::Empty => return Vec::new(),
        Shadow::All => (None, None),
        Shadow::Between(l, h, ll, hl) => (Some((*l, *ll)), Some((*h, *hl))),
        Shadow::Below(r, lab) => (None, Some((*r, *lab))),
        Shadow::Above(r, lab) => (Some((*r, *lab)), None),
    };
    let lo = match (&k.lo, &slo) {
        (None, s) => *s,
        (k, None) => *k,
        (Some(a), Some(b)) => Some(if a.0 >= b.0 { *a } else { *b }),
    };
    let hi = match (&k.hi, &shi) {
        (None, s) => *s,
        (k, None) => *k,
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { *a } else { *b }),
    };
    if let (Some(a), Some(b)) = (&lo, &hi)
        && a.0 >= b.0
    {
        return Vec::new();
    }
    vec![Comp { lo, hi }]
}

/// Subtract a shadow from a component (0, 1, or 2 results). The shadow's lower end becomes the
/// upper bound of the piece below it, and vice versa.
fn comp_subtract(k: &Comp, sh: &Shadow) -> Vec<Comp> {
    let (slo, shi): (End, End) = match sh {
        Shadow::Empty => return vec![*k],
        Shadow::All => return Vec::new(),
        Shadow::Between(l, h, ll, hl) => (Some((*l, *ll)), Some((*h, *hl))),
        Shadow::Below(r, lab) => (None, Some((*r, *lab))),
        Shadow::Above(r, lab) => (Some((*r, *lab)), None),
    };
    // No overlap → unchanged.
    let above = |x: &End, y: &End| match (x, y) {
        (Some(a), Some(b)) => a.0 >= b.0,
        _ => false,
    };
    if above(&slo, &k.hi) || above(&k.lo, &shi) {
        return vec![*k];
    }
    let mut out = Vec::new();
    // The piece below the shadow: [k.lo, shadow.lo].
    if let Some(s) = &slo {
        let keep = match &k.lo {
            None => true,
            Some(a) => a.0 < s.0,
        };
        if keep {
            out.push(Comp {
                lo: k.lo,
                hi: Some(*s),
            });
        }
    }
    // The piece above the shadow: [shadow.hi, k.hi].
    if let Some(s) = &shi {
        let keep = match &k.hi {
            None => true,
            Some(b) => s.0 < b.0,
        };
        if keep {
            out.push(Comp {
                lo: Some(*s),
                hi: k.hi,
            });
        }
    }
    out
}

/// One sample's resolved record: the hull boundary labels, the ops holing here, and the kept
/// component's µ̂-ends (float — the side consensus input).
struct SampleRec<B: Backend> {
    sigma: Rat<B>,
    lower: Label,
    upper: Label,
    hole_ops: Vec<usize>,
    mu_lo: f64,
    mu_hi: f64,
}

/// The 3-D distance² from the witness point `p` to the material of one µ̂-component at σ —
/// the quadratic `|X(σ, µ̂) − p|²` minimized over the component (`µ̂* = ⟨p − c, r⟩/|r|²`,
/// clamped into the component). The honest "keep the material near p" metric: a µ̂-midpoint
/// comparison ties on symmetric sheets, the 3-D distance does not.
fn comp_dist2<B: Backend>(
    chart: &geom::chart::Chart<B>,
    p: &[Rat<B>; 3],
    sigma: &Rat<B>,
    lo: f64,
    hi: f64,
) -> Option<f64> {
    let c = chart.pedal().eval(sigma)?;
    let r = chart.ruling().eval(sigma)?;
    let d = [p[0].sub(&c[0]), p[1].sub(&c[1]), p[2].sub(&c[2])];
    let num = d[0].mul(&r[0]).add(&d[1].mul(&r[1])).add(&d[2].mul(&r[2]));
    let den = r[0].mul(&r[0]).add(&r[1].mul(&r[1])).add(&r[2].mul(&r[2]));
    if den.sign() <= 0 {
        return None;
    }
    let mu = rat_to_f64(&num.div(&den)).clamp(lo, hi);
    let (cf, rf, pf) = (
        [rat_to_f64(&c[0]), rat_to_f64(&c[1]), rat_to_f64(&c[2])],
        [rat_to_f64(&r[0]), rat_to_f64(&r[1]), rat_to_f64(&r[2])],
        [rat_to_f64(&p[0]), rat_to_f64(&p[1]), rat_to_f64(&p[2])],
    );
    let mut acc = 0.0;
    for i in 0..3 {
        let dx = cf[i] + mu * rf[i] - pf[i];
        acc += dx * dx;
    }
    Some(acc)
}

/// Resolve one sample σ within region `ri`.
fn resolve_sample<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    forms: &[RegionForms<B>],
    ri: usize,
    sigma: Rat<B>,
) -> Result<SampleRec<B>, PartFault> {
    let mut comps = vec![Comp { lo: None, hi: None }];
    for (op, (kind, _)) in part.ops.iter().enumerate() {
        let sh = shadow_at(&forms[ri].forms[op], op, &sigma).ok_or(PartFault::Pole)?;
        let mut next = Vec::new();
        for k in &comps {
            match kind {
                OpKind::Intersect => next.extend(comp_intersect(k, &sh)),
                OpKind::Subtract => next.extend(comp_subtract(k, &sh)),
            }
        }
        comps = next;
    }
    // Stock discipline: unbounded components are not part material.
    let had_unbounded = comps.iter().any(|k| k.lo.is_none() || k.hi.is_none());
    comps.retain(|k| k.lo.is_some() && k.hi.is_some());
    if comps.is_empty() {
        return Err(if had_unbounded {
            PartFault::UnboundedRegion
        } else {
            PartFault::EmptyRegion
        });
    }
    comps.sort_by(|a, b| {
        a.lo.as_ref()
            .unwrap()
            .0
            .partial_cmp(&b.lo.as_ref().unwrap().0)
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    // Merge across gaps carved by ONE subtract op — those are interior holes — UNLESS the gap
    // crosses the chart's singular rail (`det J = 0`): components on opposite sides of it lie
    // on different sheets of the parametrization (the apex line on a cone), never one face.
    let sing: Option<f64> = {
        let m = forms[ri].detj_m.eval(&sigma).map(|v| rat_to_f64(&v));
        let c = forms[ri].detj_c.eval(&sigma).map(|v| rat_to_f64(&v));
        match (m, c) {
            (Some(m), Some(c)) if m.abs() > 1e-12 * (1.0 + c.abs()) => Some(-c / m),
            _ => None,
        }
    };
    let mut hole_ops: Vec<usize> = Vec::new();
    let mut merged: Vec<Comp> = vec![comps[0]];
    for k in comps.into_iter().skip(1) {
        let prev = merged.last_mut().expect("nonempty");
        let (gap_hi_lab, gap_lo_lab) = (prev.hi.as_ref().unwrap().1, k.lo.as_ref().unwrap().1);
        let same_sub_op =
            gap_hi_lab.0 == gap_lo_lab.0 && matches!(part.ops[gap_hi_lab.0].0, OpKind::Subtract);
        let gap = (prev.hi.as_ref().unwrap().0, k.lo.as_ref().unwrap().0);
        let crosses_sing = sing.is_some_and(|s| gap.0 < s && s < gap.1);
        if same_sub_op && !crosses_sing {
            if !hole_ops.contains(&gap_hi_lab.0) {
                hole_ops.push(gap_hi_lab.0);
            }
            prev.hi = k.hi;
        } else {
            merged.push(k);
        }
    }
    // Multiple genuinely separate components: a pick chooses, else fault.
    let chosen = if merged.len() == 1 {
        merged.remove(0)
    } else {
        match &part.pick {
            Some(RegionPick::KeepNear(p)) => {
                let mut best: Option<(f64, Comp)> = None;
                for k in merged {
                    let d2 = comp_dist2(
                        &built.charts[ri],
                        p,
                        &sigma,
                        k.lo.as_ref().unwrap().0,
                        k.hi.as_ref().unwrap().0,
                    )
                    .ok_or(PartFault::Pole)?;
                    best = match best {
                        Some((bd, bk)) if bd <= d2 => Some((bd, bk)),
                        _ => Some((d2, k)),
                    };
                }
                best.expect("nonempty").1
            }
            None => {
                // Attribute to the op whose rail separates the first two components.
                let op = merged[0].hi.as_ref().unwrap().1.0;
                return Err(PartFault::AmbiguousRegion { op });
            }
        }
    };
    hole_ops.sort_unstable();
    let (lo_end, hi_end) = (chosen.lo.unwrap(), chosen.hi.unwrap());
    Ok(SampleRec {
        sigma,
        lower: lo_end.1,
        upper: hi_end.1,
        hole_ops,
        mu_lo: lo_end.0,
        mu_hi: hi_end.0,
    })
}

/// The in-domain sweep: pull every op back on every region, resolve the sample grid, and fold
/// the records into the boundary-run structure + hole classification (see the module docs).
pub(crate) fn sweep<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
) -> Result<Structure<B>, PartFault> {
    let zero = Rat::from_i128(0);
    // Pull each op back on each region's chart.
    let mut regions: Vec<RegionForms<B>> = Vec::with_capacity(part.regions.len());
    for (r, chart) in part.regions.iter().zip(built.charts.iter()) {
        let mut forms = Vec::with_capacity(part.ops.len());
        for (op, (_, cutter)) in part.ops.iter().enumerate() {
            forms.push(
                cut_mu_form(chart, &cutter.surface(), &zero)
                    .ok_or(PartFault::CutUnresolved { op })?,
            );
        }
        let dj = chart.det_j();
        regions.push(RegionForms {
            band: r.band.clone(),
            forms,
            detj_c: dj.constant,
            detj_m: dj.mu,
        });
    }

    // The sample grid: mid-cell stations per region, plus targeted stations inside every
    // disc-positive σ-window of each subtract cylinder (so a small hole is never missed between
    // cells — and a wide gore meets a cylinder along several windows, one per ruling sheet).
    let mut samples: Vec<(usize, Rat<B>)> = Vec::new();
    // windows[op] = the disc-positive windows seen, tagged by region.
    type TaggedWindow<B> = (usize, Rat<B>, Rat<B>);
    let mut windows: Vec<Vec<TaggedWindow<B>>> = vec![Vec::new(); part.ops.len()];
    for (ri, rf) in regions.iter().enumerate() {
        let width = rf.band.hi.sub(&rf.band.lo);
        for k in 0..CELLS {
            let t = Rat::new((2 * k as i128) + 1, 2 * CELLS as i128);
            samples.push((ri, rf.band.lo.add(&width.mul(&t))));
        }
        for (op, (kind, cutter)) in part.ops.iter().enumerate() {
            if !matches!(kind, OpKind::Subtract) || !matches!(cutter, Cutter::Cylinder { .. }) {
                continue;
            }
            let roots = surface_disc_roots(&built.charts[ri], &cutter.surface(), &rf.band, 256, 60)
                .unwrap_or_default();
            for w in roots.windows(2) {
                let (t1, t2) = (&w[0], &w[1]);
                let mid = t1.add(t2).mul(&Rat::new(1, 2));
                // Only windows where the cutter is real (disc > 0 at the midpoint).
                let real = regions[ri].forms[op]
                    .disc()
                    .eval(&mid)
                    .map(|v| v.sign() > 0)
                    .unwrap_or(false);
                if !real {
                    continue;
                }
                let q1 = t1.add(&mid).mul(&Rat::new(1, 2));
                let q3 = mid.add(t2).mul(&Rat::new(1, 2));
                samples.push((ri, q1));
                samples.push((ri, mid));
                samples.push((ri, q3));
                windows[op].push((ri, t1.clone(), t2.clone()));
            }
        }
    }
    samples.sort_by(|a, b| a.1.cmp(&b.1));
    samples.dedup_by(|a, b| a.1.cmp(&b.1) == core::cmp::Ordering::Equal);

    // Resolve every sample.
    let mut recs: Vec<SampleRec<B>> = Vec::with_capacity(samples.len());
    for (ri, sigma) in samples {
        recs.push(resolve_sample(part, built, &regions, ri, sigma)?);
    }

    // Fold into runs of constant boundary labels.
    let mut runs: Vec<Run<B>> = Vec::new();
    for rec in &recs {
        match runs.last_mut() {
            Some(run) if run.lower == rec.lower && run.upper == rec.upper => {
                run.hi = rec.sigma.clone();
            }
            _ => runs.push(Run {
                lo: rec.sigma.clone(),
                hi: rec.sigma.clone(),
                lower: rec.lower,
                upper: rec.upper,
            }),
        }
    }

    // Derive roles; an op both holing and bounding is beyond this resolver — fault, don't guess.
    let mut roles = vec![OpRole::Inactive; part.ops.len()];
    let mut holes: Vec<(usize, usize, Interval<B>)> = Vec::new();
    for (op, _) in part.ops.iter().enumerate() {
        let bounds_lower = runs.iter().any(|r| r.lower.0 == op);
        let bounds_upper = runs.iter().any(|r| r.upper.0 == op);
        let holed = recs.iter().any(|r| r.hole_ops.contains(&op));
        roles[op] = if holed {
            if bounds_lower || bounds_upper {
                return Err(PartFault::AmbiguousRegion { op });
            }
            // Every disc-positive window of this op with a hole-active sample inside it is one
            // through-hole (a wrapped chart's drill can pierce the kept sheet more than once).
            // A window straddling a region join never forms (the per-band scans truncate it),
            // so an unattributable hole-active sample is refused.
            let mut attributed = 0usize;
            let mut orphaned = false;
            for rec in recs.iter().filter(|r| r.hole_ops.contains(&op)) {
                if !windows[op].iter().any(|(_, t1, t2)| {
                    t1.cmp(&rec.sigma) == core::cmp::Ordering::Less
                        && rec.sigma.cmp(t2) == core::cmp::Ordering::Less
                }) {
                    orphaned = true;
                }
            }
            for (ri, t1, t2) in &windows[op] {
                let active = recs.iter().any(|r| {
                    r.hole_ops.contains(&op)
                        && t1.cmp(&r.sigma) == core::cmp::Ordering::Less
                        && r.sigma.cmp(t2) == core::cmp::Ordering::Less
                });
                if active {
                    holes.push((
                        op,
                        *ri,
                        Interval {
                            lo: t1.clone(),
                            hi: t2.clone(),
                        },
                    ));
                    attributed += 1;
                }
            }
            if orphaned || attributed == 0 {
                return Err(PartFault::HoleCrossesRegions { op });
            }
            OpRole::Hole
        } else {
            // A bound reaches a domain end; an op bounding only interior runs bites across
            // another op's rail — a notch.
            let first = runs.first().expect("nonempty runs");
            let last = runs.last().expect("nonempty runs");
            let at_end =
                |side: &dyn Fn(&Run<B>) -> Label| side(first).0 == op || side(last).0 == op;
            match (bounds_lower, bounds_upper) {
                (true, true) => OpRole::Notch, // bites both sides somewhere
                (true, false) => {
                    if at_end(&|r: &Run<B>| r.lower) {
                        OpRole::LowerBound
                    } else {
                        OpRole::Notch
                    }
                }
                (false, true) => {
                    if at_end(&|r: &Run<B>| r.upper) {
                        OpRole::UpperBound
                    } else {
                        OpRole::Notch
                    }
                }
                (false, false) => OpRole::Inactive,
            }
        };
    }

    // The side consensus: uniform µ̂-sign across every kept sample, else undetermined.
    let mu_negative = if recs.iter().all(|r| r.mu_hi < 0.0) {
        Some(true)
    } else if recs.iter().all(|r| r.mu_lo > 0.0) {
        Some(false)
    } else {
        None
    };

    Ok(Structure {
        runs,
        holes,
        roles,
        mu_negative,
    })
}
