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

use crate::part::Extrusion;
use crate::part::{BuiltRegions, Cutter, OpKind, OpRole, Part, PartFault, RegionPick};
use develop::cut::{MuCut, cut_mu_form};
use export::approx::{f64_to_rat, rat_to_f64};
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
    /// One µ̂-root of one **wall** of an extruded cutter: `(wall index, is the upper root)`. The
    /// metric cutters have a single wall and use the three variants above, so their labels — and
    /// every golden that records one — are untouched.
    Wall(usize, bool),
}

/// A boundary label: which op's which branch bounds the kept material here.
pub(crate) type Label = (usize, BranchSide);

/// Which of the op's walls a label names — `0` for the single-walled metric cutters.
pub(crate) fn wall_of(label: Label) -> usize {
    match label.1 {
        BranchSide::Wall(i, _) => i,
        _ => 0,
    }
}

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

// Hand-written so `B` need not be `Clone` (the backend markers are not).
impl<B: Backend> Clone for Run<B> {
    fn clone(&self) -> Self {
        Run {
            lo: self.lo.clone(),
            hi: self.hi.clone(),
            lower: self.lower,
            upper: self.upper,
        }
    }
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

// Hand-written so `B` need not be `Clone` (the backend markers are not).
impl<B: Backend> Clone for Structure<B> {
    fn clone(&self) -> Self {
        Structure {
            runs: self.runs.clone(),
            holes: self.holes.clone(),
            roles: self.roles.clone(),
            mu_negative: self.mu_negative,
        }
    }
}

/// The resolver's sample-grid density per region (also the realizer's corner pad unit).
pub(crate) const CELLS: usize = 48;

/// One connected piece of a µ̂-shadow (float bounds, labels exact).
enum Patch {
    All,
    /// Inside is `µ̂ ∈ [lo, hi]`.
    Between(f64, f64, Label, Label),
    /// Inside is `µ̂ ≤ r`.
    Below(f64, Label),
    /// Inside is `µ̂ ≥ r`.
    Above(f64, Label),
}

/// The µ̂-shadow of one op at one σ: the **union** of the [`Patch`]es where that op's cutter covers
/// the ruling, in µ̂ order and pairwise disjoint.
///
/// A union, not one interval, because a general cutter's cross-section along a ruling need not be
/// connected — an extruded profile that is non-convex, or has holes, shadows the ruling in several
/// stretches. The two metric cutters are the special case of exactly one patch (a quadric `MuCut`
/// has two roots), so they cost nothing here: [`comp_intersect`] and [`comp_subtract`] still do the
/// per-patch work, and the union is a fold over them.
struct Shadow(Vec<Patch>);

impl Shadow {
    /// The empty shadow — the cutter misses this ruling entirely.
    fn empty() -> Self {
        Shadow(Vec::new())
    }
    /// A shadow of one connected piece.
    fn one(p: Patch) -> Self {
        Shadow(vec![p])
    }
    /// Intersect a component with the whole union: each patch contributes at most one component,
    /// and the patches are disjoint, so the results are too.
    fn intersect(&self, k: &Comp) -> Vec<Comp> {
        self.0.iter().flat_map(|p| comp_intersect(k, p)).collect()
    }
    /// Subtract the whole union from a component: remove each patch in turn from what survives the
    /// previous ones.
    fn subtract(&self, k: &Comp) -> Vec<Comp> {
        self.0.iter().fold(vec![*k], |acc, p| {
            acc.iter().flat_map(|c| comp_subtract(c, p)).collect()
        })
    }
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
    /// Per op, one µ̂-pullback **per wall** — a metric cutter contributes exactly one.
    pub forms: Vec<Vec<MuCut<B>>>,
    pub detj_c: lattice::RatFunc<B>,
    pub detj_m: lattice::RatFunc<B>,
}

/// The op's µ̂-shadow at σ.
///
/// A single-walled cutter's shadow is read straight off its one µ̂-quadratic, exactly as before. A
/// multi-walled one cannot be: its cross-section along the ruling is whatever the profile says, so
/// the crossings are collected from **every** wall and the stretches between them are classified by
/// the profile's own fill rule — [`Cast::contains`] at each midpoint. That is the same two-view
/// split `docs/cutter-extrude-design.md` §2.1 keeps: walls give the boundary, the region gives the
/// inside.
fn shadow_at<B: Backend>(
    cutter: &Cutter<B>,
    forms: &[MuCut<B>],
    chart: &geom::chart::Chart<B>,
    op: usize,
    sigma: &Rat<B>,
) -> Option<Shadow> {
    if let Cutter::Extrude(e) = cutter {
        return extruded_shadow(e, forms, chart, op, sigma);
    }
    let form = forms.first()?;
    let a = rat_to_f64(&form.a.eval(sigma)?);
    let b = rat_to_f64(&form.b.eval(sigma)?);
    let c = rat_to_f64(&form.c.eval(sigma)?);
    let tiny = 1e-12 * (1.0 + a.abs().max(b.abs()).max(c.abs()));
    Some(if a.abs() <= tiny {
        if b.abs() <= tiny {
            // A degenerate section (plane through the ruling): all-or-nothing by sign of c.
            if c < 0.0 {
                Shadow::one(Patch::All)
            } else {
                Shadow::empty()
            }
        } else {
            let r = -c / b;
            Shadow::one(if b > 0.0 {
                Patch::Below(r, (op, BranchSide::Plane))
            } else {
                Patch::Above(r, (op, BranchSide::Plane))
            })
        }
    } else {
        // a > 0 structurally (Cauchy–Schwarz); inside = between the roots.
        let disc = b * b - 4.0 * a * c;
        if disc <= 0.0 {
            Shadow::empty()
        } else {
            let sq = disc.sqrt();
            Shadow::one(Patch::Between(
                (-b - sq) / (2.0 * a),
                (-b + sq) / (2.0 * a),
                (op, BranchSide::Lower),
                (op, BranchSide::Upper),
            ))
        }
    })
}

/// The µ̂-shadow of an extruded cutter along one ruling.
///
/// Every wall contributes its µ̂-crossings (0, 1 or 2 roots of its quadratic). Sorted, those cut the
/// ruling into stretches, each wholly inside the profile or wholly outside — so one membership test
/// per stretch classifies it, and consecutive inside stretches are the shadow's patches. The
/// endpoints carry the wall and root that produced them, which is what lets the realizer fit the
/// right rail later.
///
/// `None` when a chart field poles or the profile's fill cannot be read at a sample (a row exact
/// ray-casting excludes) — the caller turns that into a fault rather than a guess.
fn extruded_shadow<B: Backend>(
    e: &Extrusion<B>,
    forms: &[MuCut<B>],
    chart: &geom::chart::Chart<B>,
    op: usize,
    sigma: &Rat<B>,
) -> Option<Shadow> {
    use core::cmp::Ordering;
    let cast = e.cast().ok()?;
    let zero = Rat::from_i128(0);

    // Every wall's crossings along this ruling, labelled by wall and root.
    let mut cuts: Vec<(f64, Label)> = Vec::new();
    for (wi, form) in forms.iter().enumerate() {
        let a = rat_to_f64(&form.a.eval(sigma)?);
        let b = rat_to_f64(&form.b.eval(sigma)?);
        let c = rat_to_f64(&form.c.eval(sigma)?);
        let tiny = 1e-12 * (1.0 + a.abs().max(b.abs()).max(c.abs()));
        if a.abs() <= tiny {
            if b.abs() > tiny {
                cuts.push((-c / b, (op, BranchSide::Wall(wi, false))));
            }
        } else {
            let disc = b * b - 4.0 * a * c;
            if disc > 0.0 {
                let sq = disc.sqrt();
                cuts.push(((-b - sq) / (2.0 * a), (op, BranchSide::Wall(wi, false))));
                cuts.push(((-b + sq) / (2.0 * a), (op, BranchSide::Wall(wi, true))));
            }
        }
    }
    cuts.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(Ordering::Equal));

    // Is the ruling point at this µ̂ inside the profile? Exact once the sample is chosen: the point
    // is built from the chart's own fields and the fill is the region's own even-odd rule.
    let inside = |mu: f64| -> Option<bool> {
        let mu_q = f64_to_rat::<B>(mu, 44);
        let p = chart.surface(&mu_q, &zero).eval(sigma)?;
        cast.contains(&p, &e.profile)
    };
    // A membership sample that survives the ray-casting genericity precondition: nudge and retry
    // rather than guess, and give up (fail-closed) if none of the offsets is decidable.
    // `scale` must be the width of the stretch being classified, never a global one. Membership is
    // constant *between* consecutive crossings — that is what makes one midpoint sample exact — so
    // a nudge is only sound while it stays inside that stretch. Scaled globally, a nudge on a thin
    // lobe lands in its neighbour and reports the neighbour's answer.
    let inside_near = |mu: f64, scale: f64| -> Option<bool> {
        for k in 0..4 {
            let jitter = scale * 1e-3 * (k as f64) * if k % 2 == 0 { 1.0 } else { -1.0 };
            if let Some(v) = inside(mu + jitter) {
                return Some(v);
            }
        }
        None
    };

    if cuts.is_empty() {
        // No wall meets this ruling: it is wholly inside the profile's sweep or wholly outside.
        return Some(if inside_near(0.0, 1.0)? {
            Shadow::one(Patch::All)
        } else {
            Shadow::empty()
        });
    }
    let width = (cuts[cuts.len() - 1].0 - cuts[0].0).abs().max(1.0);
    let mut patches = Vec::new();
    // The unbounded stretch below the first crossing, then each bounded stretch, then the one above.
    if inside_near(cuts[0].0 - width, width)? {
        patches.push(Patch::Below(cuts[0].0, cuts[0].1));
    }
    for pair in cuts.windows(2) {
        let (lo, hi) = (pair[0].0, pair[1].0);
        if hi - lo <= 0.0 {
            continue;
        }
        if inside_near(0.5 * (lo + hi), hi - lo)? {
            patches.push(Patch::Between(lo, hi, pair[0].1, pair[1].1));
        }
    }
    let last = cuts[cuts.len() - 1];
    if inside_near(last.0 + width, width)? {
        patches.push(Patch::Above(last.0, last.1));
    }
    Some(Shadow(patches))
}

/// Intersect a component with **one patch** of a shadow (0 or 1 result). The union form is
/// [`Shadow::intersect`], which folds this over the patches.
fn comp_intersect(k: &Comp, sh: &Patch) -> Vec<Comp> {
    let (slo, shi): (End, End) = match sh {
        Patch::All => (None, None),
        Patch::Between(l, h, ll, hl) => (Some((*l, *ll)), Some((*h, *hl))),
        Patch::Below(r, lab) => (None, Some((*r, *lab))),
        Patch::Above(r, lab) => (Some((*r, *lab)), None),
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

/// Subtract **one patch** of a shadow from a component (0, 1, or 2 results). The patch's lower end
/// becomes the upper bound of the piece below it, and vice versa. The union form is
/// [`Shadow::subtract`], which folds this over the patches.
fn comp_subtract(k: &Comp, sh: &Patch) -> Vec<Comp> {
    let (slo, shi): (End, End) = match sh {
        Patch::All => return Vec::new(),
        Patch::Between(l, h, ll, hl) => (Some((*l, *ll)), Some((*h, *hl))),
        Patch::Below(r, lab) => (None, Some((*r, *lab))),
        Patch::Above(r, lab) => (Some((*r, *lab)), None),
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

/// One merged material component at a sample: its µ̂-ends plus the subtract ops whose interior
/// gaps were merged **inside it** (its own hole record — a gap in some other component is not a
/// hole of the part).
struct MergedComp {
    comp: Comp,
    hole_ops: Vec<usize>,
}

/// The merged material components at one sample σ within region `ri` (the op-shadow interval
/// algebra + the singular-rail-guarded hole merge — no pick yet; choosing is the sweep's job).
fn sample_comps<B: Backend>(
    part: &Part<B>,
    forms: &[RegionForms<B>],
    chart: &geom::chart::Chart<B>,
    ri: usize,
    sigma: &Rat<B>,
) -> Result<Vec<MergedComp>, PartFault> {
    let mut comps = vec![Comp { lo: None, hi: None }];
    for (op, (kind, cutter)) in part.ops.iter().enumerate() {
        let sh =
            shadow_at(cutter, &forms[ri].forms[op], chart, op, sigma).ok_or(PartFault::Pole)?;
        let mut next = Vec::new();
        for k in &comps {
            match kind {
                OpKind::Intersect => next.extend(sh.intersect(k)),
                OpKind::Subtract => next.extend(sh.subtract(k)),
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
        let m = forms[ri].detj_m.eval(sigma).map(|v| rat_to_f64(&v));
        let c = forms[ri].detj_c.eval(sigma).map(|v| rat_to_f64(&v));
        match (m, c) {
            (Some(m), Some(c)) if m.abs() > 1e-12 * (1.0 + c.abs()) => Some(-c / m),
            _ => None,
        }
    };
    let mut merged: Vec<MergedComp> = vec![MergedComp {
        comp: comps[0],
        hole_ops: Vec::new(),
    }];
    for k in comps.into_iter().skip(1) {
        let prev = merged.last_mut().expect("nonempty");
        let (gap_hi_lab, gap_lo_lab) = (prev.comp.hi.as_ref().unwrap().1, k.lo.as_ref().unwrap().1);
        let same_sub_op =
            gap_hi_lab.0 == gap_lo_lab.0 && matches!(part.ops[gap_hi_lab.0].0, OpKind::Subtract);
        let gap = (prev.comp.hi.as_ref().unwrap().0, k.lo.as_ref().unwrap().0);
        let crosses_sing = sing.is_some_and(|s| gap.0 < s && s < gap.1);
        if same_sub_op && !crosses_sing {
            if !prev.hole_ops.contains(&gap_hi_lab.0) {
                prev.hole_ops.push(gap_hi_lab.0);
            }
            prev.comp.hi = k.hi;
        } else {
            merged.push(MergedComp {
                comp: k,
                hole_ops: Vec::new(),
            });
        }
    }
    Ok(merged)
}

/// Choose the kept component at every sample: **seed** where the designation is most decisive,
/// then **propagate by continuity** (σ-adjacent kept intervals of one connected face overlap).
///
/// A fixed 3-D witness alone is not enough on a wrapping window: past ~half a turn the mirror
/// nappe of a cone comes *closer* to the witness than the kept sheet that has rotated away — a
/// per-sample nearest pick flips mid-domain. The kept material is one connected face, so its
/// µ̂-component varies continuously in σ; the witness designates it **once** (at the sample with
/// the widest distance margin — right where the witness point actually lies), and overlap
/// carries the choice outward. Where several candidates overlap (a hole opening) the witness
/// re-decides among them; where **none** overlaps (a support discontinuity, or a cutter
/// outrunning the sample grid) the junction is refused as [`PartFault::AmbiguousRegion`] —
/// re-trusting the raw witness metric there is exactly the mirror-nappe hazard the seeded
/// propagation exists to avoid. With no pick at all, any multi-component sample faults
/// [`PartFault::AmbiguousRegion`] as before.
fn choose_comps<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
    at: &[(usize, Rat<B>, Vec<MergedComp>)],
) -> Result<Vec<usize>, PartFault> {
    let ends = |m: &MergedComp| -> (f64, f64) {
        (m.comp.lo.as_ref().unwrap().0, m.comp.hi.as_ref().unwrap().0)
    };
    if at.iter().all(|(_, _, comps)| comps.len() == 1) {
        return Ok(vec![0; at.len()]);
    }
    let witness = match &part.pick {
        Some(RegionPick::KeepNear(p)) => p,
        None => {
            let (_, _, comps) = at
                .iter()
                .find(|(_, _, comps)| comps.len() > 1)
                .expect("a multi-component sample exists");
            // Attribute to the op whose rail separates the first two components.
            let op = comps[0].comp.hi.as_ref().unwrap().1.0;
            return Err(PartFault::AmbiguousRegion { op });
        }
    };
    // Witness distances per sample, and the seed = the widest-margin sample (single-component
    // samples are perfect anchors).
    let mut dists: Vec<Vec<f64>> = Vec::with_capacity(at.len());
    for (ri, sigma, comps) in at {
        let mut row = Vec::with_capacity(comps.len());
        for m in comps {
            let (lo, hi) = ends(m);
            row.push(
                comp_dist2(&built.charts[*ri], witness, sigma, lo, hi).ok_or(PartFault::Pole)?,
            );
        }
        dists.push(row);
    }
    let margin_of = |row: &[f64]| -> (usize, f64) {
        let mut best = (0usize, f64::MAX);
        let mut second = f64::MAX;
        for (i, d) in row.iter().enumerate() {
            if *d < best.1 {
                second = best.1;
                best = (i, *d);
            } else if *d < second {
                second = *d;
            }
        }
        (best.0, second - best.1)
    };
    let mut seed = 0usize;
    let mut seed_margin = f64::MIN;
    for (i, row) in dists.iter().enumerate() {
        let margin = if row.len() == 1 {
            f64::INFINITY
        } else {
            margin_of(row).1
        };
        if margin > seed_margin {
            seed_margin = margin;
            seed = i;
        }
    }
    let mut chosen = vec![usize::MAX; at.len()];
    chosen[seed] = margin_of(&dists[seed]).0;
    // Propagate outward from the seed, right then left. Several overlapping candidates (a hole
    // opening) → the witness re-decides among them; **none** → refuse the junction rather than
    // re-trust the raw witness metric far from the witness (the mirror-nappe hazard).
    let step = |chosen: &[usize], from: usize, to: usize| -> Result<usize, PartFault> {
        let prev = ends(&at[from].2[chosen[from]]);
        let comps = &at[to].2;
        let overlapping: Vec<usize> = (0..comps.len())
            .filter(|&i| {
                let (lo, hi) = ends(&comps[i]);
                lo < prev.1 && prev.0 < hi
            })
            .collect();
        match overlapping.len() {
            1 => Ok(overlapping[0]),
            0 => {
                // Attribute to the op whose rail bounds the junction sample's first component.
                let op = comps[0].comp.hi.as_ref().unwrap().1.0;
                Err(PartFault::AmbiguousRegion { op })
            }
            _ => Ok(overlapping
                .into_iter()
                .min_by(|a, b| {
                    dists[to][*a]
                        .partial_cmp(&dists[to][*b])
                        .unwrap_or(core::cmp::Ordering::Equal)
                })
                .expect("nonempty components")),
        }
    };
    for i in seed + 1..at.len() {
        let pick = step(&chosen, i - 1, i)?;
        chosen[i] = pick;
    }
    for i in (0..seed).rev() {
        let pick = step(&chosen, i + 1, i)?;
        chosen[i] = pick;
    }
    Ok(chosen)
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
            let walls = cutter
                .walls()
                .map_err(|_| PartFault::CutUnresolved { op })?;
            let mut per_wall = Vec::with_capacity(walls.len());
            for wall in &walls {
                per_wall
                    .push(cut_mu_form(chart, wall, &zero).ok_or(PartFault::CutUnresolved { op })?);
            }
            forms.push(per_wall);
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
            if !matches!(kind, OpKind::Subtract) {
                continue;
            }
            // Windows belong to WALLS, not to cutter variants. A wall whose pullback is a genuine
            // quadratic (`a ≢ 0`) is real only between its tangent rulings and needs targeted
            // stations there; an affine one needs none. That criterion reproduces the old
            // `Cylinder`-only behaviour exactly — a cylinder's single wall is quadratic, a
            // half-space's is not — while letting every wall of an extruded cutter be sampled, so
            // a small feature is not dropped between cells.
            let walls = cutter
                .walls()
                .map_err(|_| PartFault::CutUnresolved { op })?;
            for (wi, wall) in walls.iter().enumerate() {
                let form = &regions[ri].forms[op][wi];
                if form.a.is_zero() {
                    continue;
                }
                let roots = surface_disc_roots(&built.charts[ri], wall, &rf.band, 256, 60)
                    .unwrap_or_default();
                for w in roots.windows(2) {
                    let (t1, t2) = (&w[0], &w[1]);
                    let mid = t1.add(t2).mul(&Rat::new(1, 2));
                    // Only windows where the wall is real (disc > 0 at the midpoint).
                    let real = form
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
    }
    samples.sort_by(|a, b| a.1.cmp(&b.1));
    samples.dedup_by(|a, b| a.1.cmp(&b.1) == core::cmp::Ordering::Equal);

    // Resolve every sample: the component algebra everywhere first, then the seeded
    // continuity-propagated choice (see [`choose_comps`]).
    let mut at: Vec<(usize, Rat<B>, Vec<MergedComp>)> = Vec::with_capacity(samples.len());
    for (ri, sigma) in samples {
        let comps = sample_comps(part, &regions, &built.charts[ri], ri, &sigma)?;
        at.push((ri, sigma, comps));
    }
    let chosen = choose_comps(part, built, &at)?;
    let mut recs: Vec<SampleRec<B>> = Vec::with_capacity(at.len());
    for ((_, sigma, comps), pick) in at.into_iter().zip(chosen) {
        let m = &comps[pick];
        let (lo_end, hi_end) = (m.comp.lo.as_ref().unwrap(), m.comp.hi.as_ref().unwrap());
        let mut hole_ops = m.hole_ops.clone();
        hole_ops.sort_unstable();
        recs.push(SampleRec {
            sigma,
            lower: lo_end.1,
            upper: hi_end.1,
            hole_ops,
            mu_lo: lo_end.0,
            mu_hi: hi_end.0,
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use develop::extrude::{Apex, Frame};
    use fixtures::devices::cone;
    use geom::content::{ArcPiece, Circle, CurveId, Edge, Half, Orient, Point2, Winding};
    use lattice::{Bignum, Surd};

    type Q = Rat<Bignum>;

    fn q(n: i128) -> Q {
        Q::from_i128(n)
    }

    /// A disc's boundary as the two x-monotone arcs `arrange2d` decomposes it into.
    fn disc_edges(cx: Q, cy: Q, r: Q, src: u32) -> Vec<Edge<Bignum>> {
        let (lo, hi) = (cx.sub(&r), cx.add(&r));
        let circle = Circle {
            cx: cx.clone(),
            cy: cy.clone(),
            r2: r.mul(&r),
        };
        [Half::Upper, Half::Lower]
            .into_iter()
            .map(|half| {
                Edge::Arc(Box::new(ArcPiece {
                    circle: circle.clone(),
                    half,
                    x_lo: Surd::from_rat(lo.clone()),
                    x_hi: Surd::from_rat(hi.clone()),
                    start: Point2::from_rat(lo.clone(), cy.clone()),
                    end: Point2::from_rat(hi.clone(), cy.clone()),
                    winding: Winding {
                        orient: Orient::Ccw,
                        source_span: None,
                    },
                    source: CurveId(src),
                }))
            })
            .collect()
    }

    /// A cutter extruding `profile` straight down the `z` axis from the `z = 0` plane.
    fn drilled(profile: Vec<Edge<Bignum>>) -> Cutter<Bignum> {
        let frame = Frame::new([q(0), q(0), q(0)], [q(1), q(0), q(0)], [q(0), q(1), q(0)])
            .expect("orthonormal frame");
        Cutter::extrude(
            frame,
            Apex::direction([q(0), q(0), q(1)]).expect("a real direction"),
            profile,
        )
    }

    /// The µ̂-shadow of a cutter on the device cone at one σ.
    fn shadow_of(cutter: &Cutter<Bignum>, sigma: &Q) -> Shadow {
        let chart = cone();
        let walls = cutter.walls().expect("well-formed cutter");
        let forms: Vec<MuCut<Bignum>> = walls
            .iter()
            .map(|w| cut_mu_form(&chart, w, &q(0)).expect("a pullback"))
            .collect();
        shadow_at(cutter, &forms, &chart, 0, sigma).expect("a decidable shadow")
    }

    fn spans(sh: &Shadow) -> Vec<(f64, f64)> {
        sh.0.iter()
            .map(|p| match p {
                Patch::Between(l, h, ..) => (*l, *h),
                Patch::Below(r, _) => (f64::NEG_INFINITY, *r),
                Patch::Above(r, _) => (*r, f64::INFINITY),
                Patch::All => (f64::NEG_INFINITY, f64::INFINITY),
            })
            .collect()
    }

    /// **The shadow the old model could not hold.** A profile of two disjoint discs strung along the
    /// ruling's own direction shadows it in *two* stretches, so the union `Shadow` of AUTH.1e.1 is
    /// load-bearing rather than decorative — a single labelled interval cannot express this, and
    /// before the refactor there was nowhere to put the second patch.
    #[test]
    fn a_two_lobed_profile_shadows_the_ruling_twice() {
        let mut profile = disc_edges(q(0), q(1), Q::new(3, 10), 0);
        profile.extend(disc_edges(q(0), Q::new(5, 2), Q::new(3, 10), 1));
        let sh = shadow_of(&drilled(profile), &q(0));
        let got = spans(&sh);
        assert_eq!(got.len(), 2, "two lobes, two patches — got {got:?}");
        // The σ = 0 ruling runs up +y at ≈0.995 µ̂ per unit, so the discs at y ∈ [0.7, 1.3] and
        // [2.2, 2.8] shadow µ̂ ≈ [0.70, 1.31] and ≈ [2.21, 2.81].
        for ((lo, hi), (want_lo, want_hi)) in got.iter().zip([(0.70, 1.31), (2.21, 2.82)]) {
            assert!(
                (lo - want_lo).abs() < 0.02 && (hi - want_hi).abs() < 0.02,
                "patch [{lo:.3}, {hi:.3}] should be ≈[{want_lo}, {want_hi}]"
            );
        }
        // And each end is labelled by the wall that made it, which is what the realizer fits.
        for p in &sh.0 {
            if let Patch::Between(_, _, a, b) = p {
                assert!(matches!(a.1, BranchSide::Wall(..)));
                assert!(matches!(b.1, BranchSide::Wall(..)));
            }
        }
    }

    /// **Differential against the path it generalizes.** One disc extruded down `z` *is* a vertical
    /// cylinder, so the new multi-wall shadow and the old single-quadric one must agree on the same
    /// geometry — computed by entirely different routes (wall crossings + the profile's fill rule,
    /// versus the roots of one µ̂-quadratic).
    #[test]
    fn an_extruded_disc_shadows_like_the_cylinder_it_is() {
        let (cy, r) = (Q::new(11, 5), Q::new(1, 5));
        let extruded = shadow_of(&drilled(disc_edges(q(0), cy.clone(), r.clone(), 0)), &q(0));
        let metric = shadow_of(&Cutter::vertical_cylinder(q(0), cy, r.mul(&r)), &q(0));
        let (a, b) = (spans(&extruded), spans(&metric));
        assert_eq!(a.len(), 1, "one disc, one patch");
        assert_eq!(b.len(), 1);
        assert!(
            (a[0].0 - b[0].0).abs() < 1e-6 && (a[0].1 - b[0].1).abs() < 1e-6,
            "extruded {a:?} should match the cylinder {b:?}"
        );
    }

    /// **The slice's real claim: an extruded cutter cuts, inside a `Part`.**
    ///
    /// The Stage-1 flex panel's interior drill `D4` is a vertical cylinder over a disc. Authored
    /// instead as that same disc *extruded* down `z`, it is the same solid — so the resolver must
    /// derive the same structure from it: the same op roles, the same interior hole over the same
    /// σ-window, the same kept side. That runs the whole new path end to end — `walls()`, the
    /// per-wall pullbacks, `extruded_shadow`, the `Wall` labels, and the per-wall stations — and
    /// compares it against the certified device it generalizes rather than against a hand-written
    /// expectation.
    #[test]
    fn an_extruded_drill_resolves_the_same_part_as_the_cylinder_it_replaces() {
        use crate::construct;
        use crate::part::SupportFn;

        // The flex panel, with D4 authored either way.
        let panel = |d4: Cutter<Bignum>| {
            construct::from_chart::<Bignum>(&cone())
                .region_sigma(q(-1), q(1), SupportFn::inherit())
                .intersect(Cutter::half_space([q(0), q(0), q(1)], q(3)))
                .subtract(Cutter::vertical_cylinder(q(0), Q::new(1, 2), q(2)))
                .subtract(Cutter::vertical_cylinder(
                    Q::new(-9, 4),
                    Q::new(9, 4),
                    Q::new(9, 16),
                ))
                .subtract(d4)
                .clearance(q(1))
        };
        let resolved = |part: Part<Bignum>| {
            let built = part.build_regions().expect("the regions develop");
            sweep(&part, &built).expect("the sweep resolves")
        };

        let metric = resolved(panel(Cutter::vertical_cylinder(
            q(0),
            Q::new(11, 5),
            Q::new(1, 25),
        )));
        let extruded = resolved(panel(drilled(disc_edges(
            q(0),
            Q::new(11, 5),
            Q::new(1, 5),
            0,
        ))));

        assert_eq!(
            extruded.roles.len(),
            metric.roles.len(),
            "same ops, so same number of derived roles"
        );
        for (i, (a, b)) in extruded.roles.iter().zip(metric.roles.iter()).enumerate() {
            assert_eq!(
                core::mem::discriminant(a),
                core::mem::discriminant(b),
                "op {i}: the extruded drill must derive the same role"
            );
        }
        assert_eq!(
            extruded.mu_negative, metric.mu_negative,
            "the kept side is a property of the solid, not of how it was authored"
        );
        assert_eq!(
            extruded.holes.len(),
            metric.holes.len(),
            "the drill is an interior hole either way"
        );
        for (a, b) in extruded.holes.iter().zip(metric.holes.iter()) {
            assert_eq!((a.0, a.1), (b.0, b.1), "same op holing in the same region");
            let (dlo, dhi) = (
                rat_to_f64(&a.2.lo) - rat_to_f64(&b.2.lo),
                rat_to_f64(&a.2.hi) - rat_to_f64(&b.2.hi),
            );
            assert!(
                dlo.abs() < 1e-3 && dhi.abs() < 1e-3,
                "the hole's σ-window should agree: extruded [{:.5}, {:.5}] vs metric [{:.5}, {:.5}]",
                rat_to_f64(&a.2.lo),
                rat_to_f64(&a.2.hi),
                rat_to_f64(&b.2.lo),
                rat_to_f64(&b.2.hi)
            );
        }
    }

    /// A **thin** lobe is still resolved, and at its true width. Membership is constant between
    /// consecutive wall crossings, so one midpoint sample per stretch is exact — but only while the
    /// sample stays in its own stretch, which is why the genericity nudge is scaled to the stretch
    /// rather than to the profile. A lobe two orders of magnitude narrower than its neighbour is
    /// where a globally-scaled nudge would read the neighbour's answer instead.
    #[test]
    fn a_thin_lobe_survives_the_membership_sampling() {
        let mut profile = disc_edges(q(0), q(1), Q::new(1, 500), 0); // r = 0.002
        profile.extend(disc_edges(q(0), Q::new(5, 2), Q::new(3, 10), 1)); // r = 0.3, 150x wider
        let sh = shadow_of(&drilled(profile), &q(0));
        let got = spans(&sh);
        assert_eq!(
            got.len(),
            2,
            "the thin lobe must not be swallowed — got {got:?}"
        );
        let (thin, fat) = (got[0].1 - got[0].0, got[1].1 - got[1].0);
        // Widths in µ̂ are the frame widths divided by the ruling's ≈0.995 xy-rate.
        assert!(
            (thin - 0.004).abs() < 5e-4,
            "the thin lobe should measure ≈0.004 in µ̂, got {thin:.6}"
        );
        assert!(
            (fat - 0.603).abs() < 5e-3,
            "the wide lobe should measure ≈0.603 in µ̂, got {fat:.6}"
        );
    }

    /// A ruling that misses the profile entirely is shadowed nowhere — the empty union, which
    /// `subtract` must leave the material untouched by.
    #[test]
    fn a_missed_profile_shadows_nothing() {
        let sh = shadow_of(&drilled(disc_edges(q(9), q(9), Q::new(1, 5), 0)), &q(0));
        assert!(sh.0.is_empty(), "got {:?}", spans(&sh));
    }
}
