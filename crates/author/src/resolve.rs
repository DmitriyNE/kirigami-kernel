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
use certify_core::Verdict;
use develop::cut::{CutSurface, EventKind, MuCut, coverage_events, cut_mu_form, tangent_events};
use develop::pick::{Sheet, Span, ray_crossings, select};
use export::approx::{f64_to_rat, rat_to_f64};
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

/// How the material's σ-extent ends on one side (`docs/cutter-extrude-design.md` §12.2).
pub(crate) enum SigmaEnd<B: Backend> {
    /// The material reaches the authored band edge — every recipe before AUTH.3, and still the
    /// common case. The extent *is* the declared domain on this side.
    Band,
    /// The material closes inside the band, in this exact isolating bracket, by these events.
    /// `pinch` distinguishes the two termination shapes §12.4 names: rails converging (a `Meet` or
    /// `Tangent`) versus a µ̂-degenerate wall flipping its coverage at full width.
    Closed {
        at: Interval<B>,
        kinds: Vec<EventKind>,
        pinch: bool,
    },
}

// Hand-written so `B` need not be `Clone` (the backend markers are not).
impl<B: Backend> Clone for SigmaEnd<B> {
    fn clone(&self) -> Self {
        match self {
            SigmaEnd::Band => SigmaEnd::Band,
            SigmaEnd::Closed { at, kinds, pinch } => SigmaEnd::Closed {
                at: at.clone(),
                kinds: kinds.clone(),
                pinch: *pinch,
            },
        }
    }
}

/// One sample's material kept **whole** (BL.1) — every µ̂-component the op algebra leaves, beside
/// the merged projection the one-interval model reads and the index it kept.
///
/// `raw` is the ground truth a boundary loop is traced over; `merged`/`pick` are the lossy view
/// every consumer uses today. Keeping both is what makes the one-interval path *derived* rather
/// than the only thing computed — BL.2 walks `raw`, and until it does, nothing reads it, which is
/// why this slice is gated byte-identical.
pub(crate) struct Cell<B: Backend> {
    /// The sample σ.
    pub sigma: Rat<B>,
    /// Which region the sample sits in.
    pub region: usize,
    /// Every component here, in µ̂ order, gaps and all.
    pub raw: Vec<Comp>,
    /// The same material after gaps carved by one subtract op are folded in as holes.
    pub merged: Vec<MergedComp>,
    /// The merged component the sweep kept.
    pub pick: usize,
}

// Hand-written so `B` need not be `Clone` (the backend markers are not).
impl<B: Backend> Clone for Cell<B> {
    fn clone(&self) -> Self {
        Cell {
            sigma: self.sigma.clone(),
            region: self.region,
            raw: self.raw.clone(),
            merged: self.merged.clone(),
            pick: self.pick,
        }
    }
}

/// The resolved structure the realizer certifies.
pub(crate) struct Structure<B: Backend> {
    /// The material's **derived** σ-extent — the sub-interval of the authored domain the ops
    /// actually leave material over. Equal to the authored domain unless a cutter terminates the
    /// blank, which is the AUTH.3 case; the runs below cover exactly this.
    pub domain: Interval<B>,
    /// How the extent ends, lower then upper.
    pub ends: [SigmaEnd<B>; 2],
    /// The boundary runs, in σ order, covering the derived domain.
    pub runs: Vec<Run<B>>,
    /// Ops classified as interior holes: `(op, region, the disc-positive σ-window)`.
    pub holes: Vec<(usize, usize, Interval<B>)>,
    /// The derived role per op.
    pub roles: Vec<OpRole>,
    /// The kept material's µ̂-side, when it is uniform across every sample: `Some(true)` = all
    /// µ̂ < 0, `Some(false)` = all µ̂ > 0, `None` = mixed or 0-straddling. The fold's side
    /// convention (derived, never authored — the seam-#3 doctrine).
    pub mu_negative: Option<bool>,
    /// Every live sample's material, unprojected (BL.1). `runs`/`holes`/`roles` above are the
    /// one-interval reading of exactly this; BL.2 traces boundary loops over it instead.
    pub cells: Vec<Cell<B>>,
}

// Hand-written so `B` need not be `Clone` (the backend markers are not).
impl<B: Backend> Clone for Structure<B> {
    fn clone(&self) -> Self {
        Structure {
            domain: self.domain.clone(),
            ends: [self.ends[0].clone(), self.ends[1].clone()],
            runs: self.runs.clone(),
            holes: self.holes.clone(),
            roles: self.roles.clone(),
            mu_negative: self.mu_negative,
            cells: self.cells.clone(),
        }
    }
}

/// The resolver's sample-grid density per region (also the realizer's corner pad unit).
pub(crate) const CELLS: usize = 48;

/// How tightly a wall's tangent rulings are bracketed when its σ-windows are derived:
/// `2⁻⁴⁰ ≈ 9·10⁻¹³`.
///
/// A window is read as the **gap between two brackets**, so this is how much of the window's own
/// ends the derivation gives up — three orders below the `2⁻³⁰` grid every emitted vertex is
/// snapped to, and so invisible in the artifact, while any looser choice starts to trim a hole's
/// tangent tips visibly. It is not an accuracy knob in the old sense: [`tangent_events`] isolates
/// every root first and bisects a bracket it has already proved to contain one, so no window is
/// *missed* however coarse this is — the sampling that could miss one is what it replaced.
pub(crate) fn tangent_tol<B: Backend>() -> Rat<B> {
    Rat::new(1, 1i128 << 40)
}

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
pub(crate) struct Comp {
    pub lo: End,
    pub hi: End,
}

/// The per-op pullbacks on one region's chart, plus the chart's **singular rail** — the µ̂ where
/// `det J = 0` at `w = 0` (`µ̂ₛ(σ) = −(c′·n′)/(r′·n′)`, exact rational; the apex line on a cone).
/// Material components on opposite sides of it lie on different sheets of the parametrization,
/// so a gap crossing it is never an interior hole.
pub(crate) struct RegionForms<B: Backend> {
    pub band: Interval<B>,
    /// Per op, one µ̂-pullback **per wall** — a metric cutter contributes exactly one.
    pub forms: Vec<Vec<MuCut<B>>>,
    /// The same pullbacks in **one flat list, across every op** — the argument list the σ-extent's
    /// ends are located from ([`locate_end`]). Flat rather than per-op because the two walls that
    /// close the kept µ̂-interval need not belong to the same cutter: a contour's band can slide
    /// entirely inside a subtract's carve, and the end is then the crossing of their two rails
    /// (`docs/cutter-extrude-design.md` §12.2). No fixture exercises that case yet — the ones that
    /// exist close on the contour's own walls — so it is a soundness requirement rather than a
    /// measured one, and cheap either way.
    pub flat: Vec<MuCut<B>>,
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
    let n = cuts.len();

    // Classify each stretch between consecutive crossings: `Some(true)` inside the profile's sweep,
    // `Some(false)` outside, `None` for a degenerate stretch (two walls crossing at the same µ̂),
    // which has no interior to sample and must not break a run. Stretch `0` is `(−∞, cuts[0])`,
    // stretch `k` is `(cuts[k−1], cuts[k])`, and stretch `n` is `(cuts[n−1], +∞)`.
    let mut flags: Vec<Option<bool>> = Vec::with_capacity(n + 1);
    flags.push(Some(inside_near(cuts[0].0 - width, width)?));
    for pair in cuts.windows(2) {
        let (lo, hi) = (pair[0].0, pair[1].0);
        if hi - lo <= 0.0 {
            flags.push(None);
        } else {
            flags.push(Some(inside_near(0.5 * (lo + hi), hi - lo)?));
        }
    }
    flags.push(Some(inside_near(cuts[n - 1].0 + width, width)?));

    // **Consecutive inside stretches are ONE patch.** `cuts` carries every wall's crossings, and a
    // wall crosses wherever its whole carrier does — not only where it bounds. So a ruling through
    // a rim-with-a-lug meets the rim circle *inside* the lug, where the profile is filled on both
    // sides, and the inside stretches either side of that crossing are one piece of material.
    //
    // Emitting them as two patches is set-theoretically harmless — their union is the same, and
    // `comp_subtract` folded over abutting patches gives the same answer as over their union — but
    // it is **not** harmless for `Intersect`: `comp_intersect` yields one component per patch, and
    // `sample_comps` merges components only across a gap carved by a single `Subtract`. Abutting
    // intersect pieces therefore survived as separate components, and the component pick kept only
    // the one holding the witness. On the device's rim that dropped the lug and left the boundary
    // on the nose wall's *near* root, cutting the tab inward as a bite (#296).
    let mut patches = Vec::new();
    let mut k = 0usize;
    while k <= n {
        if flags[k] != Some(true) {
            k += 1;
            continue;
        }
        // Extend over every stretch that is inside or degenerate; a degenerate one adds no width,
        // so including a trailing run of them cannot enlarge the patch.
        let start = k;
        let mut end = k;
        let mut j = k + 1;
        while j <= n && flags[j] != Some(false) {
            if flags[j] == Some(true) {
                end = j;
            }
            j += 1;
        }
        let lo = (start > 0).then(|| cuts[start - 1]);
        let hi = (end < n).then(|| cuts[end]);
        patches.push(match (lo, hi) {
            (None, None) => Patch::All,
            (None, Some((r, lab))) => Patch::Below(r, lab),
            (Some((r, lab)), None) => Patch::Above(r, lab),
            (Some((l, ll)), Some((h, hl))) => Patch::Between(l, h, ll, hl),
        });
        k = end + 1;
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
#[derive(Clone)]
pub(crate) struct MergedComp {
    pub comp: Comp,
    pub hole_ops: Vec<usize>,
}

/// The material at one sample σ, in **both** readings (BL.1).
///
/// `raw` is what the op algebra actually leaves — every µ̂-component, gaps and all. `merged` is the
/// projection the one-interval model consumes: consecutive components whose gap one subtract op
/// carved are folded together, that op recorded as a hole of the survivor. The projection is lossy
/// exactly where BL is: a gap that reaches the material's edge is not a hole, and a component the
/// pick discards has a boundary the part still owns. Keeping `raw` beside `merged` costs one clone
/// of a `Copy` list per sample and is what BL.2 traces loops over; every consumer today reads
/// `merged`, so the derived structure is unchanged.
struct Section {
    raw: Vec<Comp>,
    merged: Vec<MergedComp>,
}

impl Section {
    /// Whether this σ carries no material at all — the emptiness the σ-extent is derived from.
    fn is_empty(&self) -> bool {
        self.merged.is_empty()
    }
}

/// The merged material components at one sample σ within region `ri` (the op-shadow interval
/// algebra + the singular-rail-guarded hole merge — no pick yet; choosing is the sweep's job).
#[allow(clippy::too_many_arguments)]
fn sample_comps<B: Backend>(
    part: &Part<B>,
    forms: &[RegionForms<B>],
    chart: &geom::chart::Chart<B>,
    reach: &[Option<Vec<usize>>],
    ri: usize,
    sigma: &Rat<B>,
) -> Result<Section, PartFault> {
    let mut comps = vec![Comp { lo: None, hi: None }];
    for (op, (kind, cutter)) in part.ops.iter().enumerate() {
        // An op whose span does not reach this region is not applied here at all — the correct
        // no-op for a `Subtract` (removes nothing) and for an `Intersect` (restricts nothing).
        if reach[op].as_ref().is_some_and(|rs| !rs.contains(&ri)) {
            continue;
        }
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
    //
    // The two ways this can come out empty are **not** the same thing, and separating them is what
    // lets σ become a derived extent (`docs/cutter-extrude-design.md` §12.1). Material that is
    // *absent* — every op's shadow left nothing, which is what an intersect does past its own
    // footprint — puts this σ outside the part, and the sweep records that rather than faulting.
    // Material that is *unbounded* is a stock error at this σ: it is not missing, it is unbounded,
    // and trimming the extent to where the blank happens to be bounded would ship a part whose end
    // is the unboundedness frontier rather than a cut. So that one stays a refusal.
    let had_unbounded = comps.iter().any(|k| k.lo.is_none() || k.hi.is_none());
    comps.retain(|k| k.lo.is_some() && k.hi.is_some());
    if comps.is_empty() {
        if had_unbounded {
            return Err(PartFault::UnboundedRegion);
        }
        return Ok(Section {
            raw: Vec::new(),
            merged: Vec::new(),
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
    let raw: Vec<Comp> = comps.clone();
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
    Ok(Section { raw, merged })
}

/// Where the material's σ-extent ends on one side, located **exactly** — the AUTH.3a derivation
/// (`docs/cutter-extrude-design.md` §12.2).
///
/// `live` is the outermost σ that carries material and `dead` the neighbouring one that does not
/// (a sample, or the authored band edge). The end lies between them, and it is a **root**, not a
/// width to bisect: where material stops, the wall bounding the kept µ̂-interval from below and the
/// wall bounding it from above cross the ruling at a common µ̂ — `Res_µ̂ = 0` for two different
/// walls, `disc_µ̂ = 0` for one wall's own two roots — or a µ̂-degenerate wall flips its coverage at
/// a root of `c` ([`coverage_events`]). All three are already exact and Sturm-isolated.
///
/// Which of several candidate brackets actually closes the material is **not** decided by
/// proximity, and the fixtures show why: the quadric contour's end-cell holds two candidates
/// `2·10⁻³` apart, and the nearer one ends nothing — it is where the *lower* bound hands over from
/// the panel's carve to the contour's own lower root, after which both bounds are the contour's and
/// the interval narrows for another `2·10⁻³` before pinching shut. The brackets are *isolating*, so
/// each gap between two of them is proved free of structural change and one evaluation of the
/// component algebra decides that whole gap. That is #268's recipe one level out — the reason a
/// search is not needed and would not be sound.
///
/// The returned bracket's **inner** edge becomes the domain end, so the realized part stops at
/// most one bracket width (`2⁻⁴⁰ ≈ 9·10⁻¹³`) short of the true end — three orders below the `2⁻³⁰`
/// grid every emitted vertex is snapped to, and on the side that keeps the rails real.
///
/// Also returns **the live σ it evaluated**, which the sweep folds back into its sample set. Those
/// are not a by-product: the stretch between the outermost grid sample and the end holds no station
/// at all, and it is where the structure changes most — on the quadric fixture the lower bound
/// hands over from the panel's carve to the contour's own lower root inside it. Resolved without
/// them, the boundary would carry the *carve* as its lower rail right up to a pinch the carve has
/// nothing to do with. One evaluation per gap is exactly the right density, because a gap between
/// isolating brackets is where the structure does not change.
#[allow(clippy::too_many_arguments)]
fn locate_end<B: Backend>(
    part: &Part<B>,
    forms: &[RegionForms<B>],
    chart: &geom::chart::Chart<B>,
    reach: &[Option<Vec<usize>>],
    ri: usize,
    live: &Rat<B>,
    dead: &Rat<B>,
) -> Result<(SigmaEnd<B>, Vec<Rat<B>>), PartFault> {
    use core::cmp::Ordering;
    let ascending = live.cmp(dead) == Ordering::Less;
    let window = if ascending {
        Interval {
            lo: live.clone(),
            hi: dead.clone(),
        }
    } else {
        Interval {
            lo: dead.clone(),
            hi: live.clone(),
        }
    };
    let tol = tangent_tol::<B>();

    // Every candidate in the window, from all three families, over every op's walls at once.
    let mut zones: Vec<(Interval<B>, Vec<EventKind>, bool)> =
        develop::cut::structure_events(&forms[ri].flat, &window, &tol)
            .map_err(|_| PartFault::SigmaEndUnattributed)?
            .into_iter()
            .map(|e| {
                let pinch = e
                    .kinds
                    .iter()
                    .any(|k| matches!(k, EventKind::Meet(..) | EventKind::Tangent(..)));
                (e.at, e.kinds, pinch)
            })
            .collect();
    // The third class, folded in so the derivation matches its own claim rather than the cases a
    // fixture happens to reach — and **unreachable today**, measured rather than assumed. A wall
    // degenerate in µ̂ needs `n ⊥ ruling(σ)` for every σ, so the rulings must all point one way:
    // a cylinder chart. The cylinder the fixtures ship has `h ≡ 0`, which puts its whole `w = 0`
    // surface in one plane and leaves `c` constant (no root, no flip); give it a moving support and
    // `c` does vary — measured `2.2, 0, −1, −2, −4.2` with a root at `σ = −1` — but the chart is
    // then `NotDevelopable`, refused a step earlier. So the branch below is correct, cheap and
    // presently dead, and the restriction that makes it dead is in the development tier, not here.
    for form in &forms[ri].flat {
        for at in
            coverage_events(form, &window, &tol).map_err(|_| PartFault::SigmaEndUnattributed)?
        {
            // A coverage flip carries no `EventKind` — it is precisely the change none of the three
            // structure families names — so it is recorded as an empty kind list and `pinch = false`
            // (the material ends at full width, a jump rather than a pinch: §12.4).
            zones.push((at, Vec::new(), false));
        }
    }
    // Merge overlapping brackets, exactly as `structure_events` does within its own families: the
    // partition must stay a partition even where two candidates land closer than `tol`, or the gap
    // between them is degenerate and decides nothing.
    zones.sort_by(|x, y| x.0.lo.cmp(&y.0.lo));
    let mut merged: Vec<(Interval<B>, Vec<EventKind>, bool)> = Vec::with_capacity(zones.len());
    for (at, kinds, pinch) in zones {
        match merged.last_mut() {
            Some(prev) if at.lo.cmp(&prev.0.hi) != Ordering::Greater => {
                if at.hi.cmp(&prev.0.hi) == Ordering::Greater {
                    prev.0.hi = at.hi;
                }
                prev.1.extend(kinds);
                prev.2 |= pinch;
            }
            _ => merged.push((at, kinds, pinch)),
        }
    }
    if !ascending {
        merged.reverse();
    }

    // Walk the gaps from the live side: `live → b₀ → b₁ → … → dead`. Each gap is structure-free,
    // so one midpoint evaluation decides the whole of it, and the first **dead** gap names the
    // bracket just crossed. A degenerate gap (two brackets touching after the merge) decides
    // nothing and is skipped, which only defers the decision to the next one.
    let n = merged.len();
    if n == 0 {
        return Err(PartFault::SigmaEndUnattributed);
    }
    let near_of = |i: usize| {
        let at = &merged[i].0;
        if ascending {
            at.lo.clone()
        } else {
            at.hi.clone()
        }
    };
    let far_of = |i: usize| {
        let at = &merged[i].0;
        if ascending {
            at.hi.clone()
        } else {
            at.lo.clone()
        }
    };
    let alive = |sigma: &Rat<B>| -> Result<bool, PartFault> {
        Ok(!sample_comps(part, forms, chart, reach, ri, sigma)?.is_empty())
    };
    let mut prev = live.clone();
    let mut closed: Option<usize> = None;
    let mut probes: Vec<Rat<B>> = Vec::new();
    for i in 0..=n {
        let stop = if i < n { near_of(i) } else { dead.clone() };
        if prev.cmp(&stop) != Ordering::Equal {
            let mid = prev.add(&stop).mul(&Rat::new(1, 2));
            if !alive(&mid)? {
                if i == 0 {
                    // Dead before any candidate: the end is one the window did not contain, so
                    // refuse rather than name an event that did not close it.
                    return Err(PartFault::SigmaEndUnattributed);
                }
                closed = Some(i - 1);
                break;
            }
            probes.push(mid);
        }
        if i < n {
            prev = far_of(i);
        }
    }
    // Every gap live or degenerate: `dead` is dead by the caller's contract, so the transition
    // happened across the last bracket.
    let (at, kinds, pinch) = merged.swap_remove(closed.unwrap_or(n - 1));
    Ok((SigmaEnd::Closed { at, kinds, pinch }, probes))
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
    at: &[(usize, Rat<B>, Section)],
) -> Result<Vec<usize>, PartFault> {
    let ends = |m: &MergedComp| -> (f64, f64) {
        (m.comp.lo.as_ref().unwrap().0, m.comp.hi.as_ref().unwrap().0)
    };
    if at.iter().all(|(_, _, sec)| sec.merged.len() == 1) {
        return Ok(vec![0; at.len()]);
    }
    let witness = match &part.pick {
        Some(RegionPick::KeepNear(p)) => p,
        None => {
            let (_, _, sec) = at
                .iter()
                .find(|(_, _, sec)| sec.merged.len() > 1)
                .expect("a multi-component sample exists");
            // Attribute to the op whose rail separates the first two components.
            let op = sec.merged[0].comp.hi.as_ref().unwrap().1.0;
            return Err(PartFault::AmbiguousRegion { op });
        }
    };
    // Witness distances per sample, and the seed = the widest-margin sample (single-component
    // samples are perfect anchors).
    let mut dists: Vec<Vec<f64>> = Vec::with_capacity(at.len());
    for (ri, sigma, sec) in at {
        let mut row = Vec::with_capacity(sec.merged.len());
        for m in &sec.merged {
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
        let prev = ends(&at[from].2.merged[chosen[from]]);
        let comps = &at[to].2.merged;
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

/// Which regions each op's cut actually reaches, by index — `None` for an op that reaches all of
/// them, which is every metric cutter and every extrusion spanning `Through`.
///
/// The span counts crossings of the part's **neutral surfaces** along the extrusion's own reference
/// ray, ordered by ray parameter (`develop::pick::ray_crossings`), and `select` takes the ones the
/// span reaches. An op that does not reach a region is simply **not applied** there, which is the
/// right no-op for both kinds: a `Subtract` removes nothing, an `Intersect` restricts nothing.
///
/// **Known limitation.** The crossing search uses each region's full µ̂ extent, so it counts
/// crossings of the *surface*, not of the trimmed material — a ray that leaves the material and
/// re-crosses the surface's untrimmed continuation still counts one. That matches §5's wording
/// ("neutral surfaces", "chart embeddings"), and deriving the material extent instead is circular:
/// it depends on the very ops the span restricts.
fn span_reach<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
) -> Result<Vec<Option<Vec<usize>>>, PartFault> {
    let mut out = Vec::with_capacity(part.ops.len());
    for (op, (_, cutter)) in part.ops.iter().enumerate() {
        let Cutter::Extrude(e) = cutter else {
            out.push(None);
            continue;
        };
        if matches!(e.span, Span::Through) {
            out.push(None);
            continue;
        }
        // Every region is a sheet: its own chart (hence its own support) over its authored σ-band.
        let wide = Interval {
            lo: Rat::from_i128(-1_000_000),
            hi: Rat::from_i128(1_000_000),
        };
        let sheets: Vec<Sheet<'_, B>> = part
            .regions
            .iter()
            .zip(built.charts.iter())
            .map(|(r, chart)| Sheet {
                chart,
                sigma: r.band.clone(),
                mu: wide.clone(),
            })
            .collect();
        let crossings = match ray_crossings(
            &sheets,
            &Rat::from_i128(0),
            &e.reference_ray().ok_or(PartFault::CutUnresolved { op })?,
            &part.clearance,
            48,
        ) {
            Verdict::Verified(c) => c,
            // An unorderable or ungrounded cast cannot name an ordinal, so the cut is refused
            // rather than silently applied everywhere.
            _ => return Err(PartFault::CutUnresolved { op }),
        };
        let reached = select(e.span, &crossings).map_err(|_| PartFault::CutUnresolved { op })?;
        out.push(Some(reached.iter().map(|c| c.sheet).collect()));
    }
    Ok(out)
}

/// The in-domain sweep: pull every op back on every region, resolve the sample grid, and fold
/// the records into the boundary-run structure + hole classification (see the module docs).
pub(crate) fn sweep<B: Backend>(
    part: &Part<B>,
    built: &BuiltRegions<B>,
) -> Result<Structure<B>, PartFault> {
    let zero = Rat::from_i128(0);
    let reach = span_reach(part, built)?;
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
        let flat: Vec<MuCut<B>> = forms.iter().flatten().cloned().collect();
        let dj = chart.det_j();
        regions.push(RegionForms {
            band: r.band.clone(),
            forms,
            flat,
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
        for (op, (_kind, cutter)) in part.ops.iter().enumerate() {
            // Both senses are targeted, and that is AUTH.3a's second change. Station targeting was
            // `Subtract`-only, so an intersect's footprint was never sampled deliberately and the
            // uniform grid does not find it by luck — a square subtending ≈0.128 in σ is narrower
            // than one cell of a `±3.5` band, and not one of the 48 samples lands inside it. A
            // σ-extent derived from a grid that cannot see the contour comes back empty for a cause
            // unrelated to the geometry: fail-closed but misattributed, which is what #268 was.
            // Nothing else moves — hole attribution reads `hole_ops`, which only a `Subtract` can
            // populate, so an intersect can never be classified `Hole` by gaining stations here.
            // Windows belong to WALLS, not to cutter variants. A wall whose pullback is a genuine
            // quadratic (`a ≢ 0`) is real only between its tangent rulings and needs targeted
            // stations there; an affine one needs none. That criterion reproduces the old
            // `Cylinder`-only behaviour exactly — a cylinder's single wall is quadratic, a
            // half-space's is not — while letting every wall of an extruded cutter be sampled, so
            // a small feature is not dropped between cells.
            let walls = cutter
                .walls()
                .map_err(|_| PartFault::CutUnresolved { op })?;
            // The targeted windows have to cover the **whole** profile, and the quadric walls'
            // tangent windows only cover the quadric part of it. An all-affine profile has no
            // window at all, so a polygonal slot would be dropped between sample cells (the §6
            // failure); a *mixed* profile is the same defect one step in — the keyhole's circle
            // stops where its head does and the stem runs past, so the footprint reached the scan's
            // own edge and the tracer refused the cut as `ShadowUnbounded`. Either way the profile's
            // bounding circle supplies one window covering everything, and a superset is the right
            // error: extra stations sample where the cut is absent and cost nothing, while a
            // missing one loses the cut silently. A profile whose walls are *all* quadric needs no
            // proxy — each wall's window covers its own arc, and together they cover the profile.
            let quadric: Vec<usize> = (0..walls.len())
                .filter(|wi| !regions[ri].forms[op][*wi].a.is_zero())
                .collect();
            let bound = if quadric.len() < walls.len() {
                match cutter {
                    Cutter::Extrude(e) => e.bounding_wall(),
                    _ => None,
                }
            } else {
                None
            };
            // Stations and hole-attribution windows are two different obligations, and only the
            // proxy case separates them. **Stations** must reach every wall a boundary can hand
            // off to: a mixed profile's quadric wall bounds only inside its own tangent window,
            // and the drawing's R 0.3 root fillet subtends ~1.2 cells — probed through the proxy
            // alone it gets a run only by grid luck, and a corner missing its run cannot splice
            // (the two-walls-between refusal). **Windows** are what a subtract's hole loops are
            // attributed to and traced over, so the mixed case must keep exactly the proxy's — a
            // second, per-arc window over the same bore would trace the same hole twice.
            let probes: Vec<(&CutSurface<B>, bool)> = match &bound {
                Some(b) => core::iter::once((b, true))
                    .chain(quadric.iter().map(|wi| (&walls[*wi], false)))
                    .collect(),
                None => quadric.iter().map(|wi| (&walls[*wi], true)).collect(),
            };
            for (wall, record_window) in probes {
                // The probe's **own** pullback decides which of its root brackets are real windows.
                // Reading a wall-indexed form instead was right only while the proxy appeared for
                // all-affine profiles alone: with a mixed one it filtered the proxy's brackets by
                // the circle's reality, which is a different surface.
                let form = cut_mu_form(&built.charts[ri], wall, &zero)
                    .ok_or(PartFault::CutUnresolved { op })?;
                let brackets = tangent_events(&form, &rf.band, &tangent_tol())
                    .map_err(|_| PartFault::CutUnresolved { op })?;
                // The band cut at every bracket — including the two stretches the **band's own
                // ends** make. Reading `brackets.windows(2)` named only the stretches between two
                // tangent rulings, so a footprint that reaches the band's edge got no window at
                // all, and its hole-active samples were orphaned into `HoleCrossesRegions` — a
                // fault about region joins, reported for a cut that never crossed one. That is
                // #302's blind spot one layer up: there it cost `window_for` its clamp on rail
                // windows, here it costs a hole its very attribution. Sound for the same reason the
                // interior stretches are: the brackets isolate every root of `disc` in the band, so
                // each stretch carries one sign and its own midpoint decides it.
                let mut stretches: Vec<(Rat<B>, Rat<B>)> = Vec::with_capacity(brackets.len() + 1);
                let mut open = rf.band.lo.clone();
                for b in &brackets {
                    stretches.push((core::mem::replace(&mut open, b.hi.clone()), b.lo.clone()));
                }
                stretches.push((open, rf.band.hi.clone()));
                for (t1, t2) in &stretches {
                    // Each bracket **contains** its tangent ruling, so the gap between two of them
                    // lies inside the true window and the discriminant has one sign across it: a
                    // single evaluation at the midpoint decides the whole gap.
                    if t1.cmp(t2) != core::cmp::Ordering::Less {
                        continue; // a bracket straddling the band's end leaves no stretch
                    }
                    let mid = t1.add(t2).mul(&Rat::new(1, 2));
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
                    if record_window {
                        windows[op].push((ri, t1.clone(), t2.clone()));
                    }
                }
            }
        }
    }
    samples.sort_by(|a, b| a.1.cmp(&b.1));
    samples.dedup_by(|a, b| a.1.cmp(&b.1) == core::cmp::Ordering::Equal);

    // Resolve every sample: the component algebra everywhere first, then the seeded
    // continuity-propagated choice (see [`choose_comps`]). A sample may now come back with **no**
    // components — that σ simply carries no material — so the extent is derived before the choice.
    // BL.1: each sample keeps BOTH readings — `sec.raw` is every component the op algebra leaves,
    // `sec.merged` the one-interval-plus-holes projection every consumer reads today.
    let mut all: Vec<(usize, Rat<B>, Section)> = Vec::with_capacity(samples.len());
    for (ri, sigma) in samples {
        let sec = sample_comps(part, &regions, &built.charts[ri], &reach, ri, &sigma)?;
        all.push((ri, sigma, sec));
    }

    // — The derived σ-extent (AUTH.3a, `docs/cutter-extrude-design.md` §12). —
    let (first, last) = {
        let live = |i: &usize| !all[*i].2.is_empty();
        let lo = (0..all.len()).find(live);
        let hi = (0..all.len()).rev().find(live);
        match (lo, hi) {
            (Some(a), Some(b)) => (a, b),
            _ => return Err(PartFault::EmptyRegion),
        }
    };
    // A part is one connected piece. Two live runs with a gap between them is not a part the
    // resolver may pick from, so it refuses rather than choosing one or emitting both.
    if all[first..=last].iter().any(|(_, _, c)| c.is_empty()) {
        return Err(PartFault::DisconnectedRegion);
    }
    // Each end is either the authored band edge or a located event. The band edges themselves are
    // never sample stations (the grid is mid-cell), so they are probed here rather than assumed:
    // "no dead sample below the first live one" is not the same claim as "the material reaches the
    // edge", and the difference is exactly a contour that dies inside the first half-cell.
    let outer_live = |edge: &Rat<B>, ri: usize| -> Result<bool, PartFault> {
        Ok(!sample_comps(part, &regions, &built.charts[ri], &reach, ri, edge)?.is_empty())
    };
    let band_lo = regions[0].band.lo.clone();
    let band_hi = regions[regions.len() - 1].band.hi.clone();
    let (ri_lo, ri_hi) = (all[first].0, all[last].0);
    let mut probes: Vec<(usize, Rat<B>)> = Vec::new();
    let mut end_at = |ri: usize, live: &Rat<B>, dead: &Rat<B>| -> Result<SigmaEnd<B>, PartFault> {
        let (end, found) = locate_end(part, &regions, &built.charts[ri], &reach, ri, live, dead)?;
        probes.extend(found.into_iter().map(|s| (ri, s)));
        Ok(end)
    };
    let lower = if first > 0 {
        end_at(ri_lo, &all[first].1, &all[first - 1].1)?
    } else if outer_live(&band_lo, ri_lo)? {
        SigmaEnd::Band
    } else {
        end_at(ri_lo, &all[first].1, &band_lo)?
    };
    let upper = if last + 1 < all.len() {
        end_at(ri_hi, &all[last].1, &all[last + 1].1)?
    } else if outer_live(&band_hi, ri_hi)? {
        SigmaEnd::Band
    } else {
        end_at(ri_hi, &all[last].1, &band_hi)?
    };
    // The bracket's **inner** edge is the domain end — inside the material, where the rails are
    // still real, and short of the true end by at most one bracket width (`2⁻⁴⁰`).
    let domain = Interval {
        lo: match &lower {
            SigmaEnd::Band => band_lo,
            SigmaEnd::Closed { at, .. } => at.hi.clone(),
        },
        hi: match &upper {
            SigmaEnd::Band => band_hi,
            SigmaEnd::Closed { at, .. } => at.lo.clone(),
        },
    };

    // Fold the end-locator's own live evaluations back in as samples. The stretch between the
    // outermost grid station and a derived end carries **no** station — the grid is uniform and the
    // end is wherever the ops put it — and it is exactly where the structure turns over: on the
    // quadric fixture the lower bound hands off from the panel's carve to the contour's own lower
    // root inside it. Without these the boundary would run the carve's rail into a pinch the carve
    // has nothing to do with, and the loop would not close. They cost nothing: they are evaluations
    // the derivation already made.
    if !probes.is_empty() {
        for (ri, sigma) in probes {
            let comps = sample_comps(part, &regions, &built.charts[ri], &reach, ri, &sigma)?;
            all.push((ri, sigma, comps));
        }
        all.sort_by(|a, b| a.1.cmp(&b.1));
        all.dedup_by(|a, b| a.1.cmp(&b.1) == core::cmp::Ordering::Equal);
    }
    let live = |i: &usize| !all[*i].2.is_empty();
    let first = (0..all.len()).find(live).ok_or(PartFault::EmptyRegion)?;
    let last = (0..all.len())
        .rev()
        .find(live)
        .ok_or(PartFault::EmptyRegion)?;

    let at = &all[first..=last];
    let chosen = choose_comps(part, built, at)?;
    let mut recs: Vec<SampleRec<B>> = Vec::with_capacity(at.len());
    let mut cells: Vec<Cell<B>> = Vec::with_capacity(at.len());
    for ((ri, sigma, sec), pick) in at.iter().zip(chosen) {
        cells.push(Cell {
            sigma: sigma.clone(),
            region: *ri,
            raw: sec.raw.clone(),
            merged: sec.merged.clone(),
            pick,
        });
        let m = &sec.merged[pick];
        let (lo_end, hi_end) = (m.comp.lo.as_ref().unwrap(), m.comp.hi.as_ref().unwrap());
        let mut hole_ops = m.hole_ops.clone();
        hole_ops.sort_unstable();
        recs.push(SampleRec {
            sigma: sigma.clone(),
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
    // Derive roles. An op that both *carves a gap* and *bounds* is the one-interval model's own
    // frontier, not an ambiguity — see [`PartFault::SectionNotSimple`] for what it means and why
    // merging the two stretches anyway would ship the wrong pattern.
    let mut roles = vec![OpRole::Inactive; part.ops.len()];
    let mut holes: Vec<(usize, usize, Interval<B>)> = Vec::new();
    for (op, _) in part.ops.iter().enumerate() {
        let bounds_lower = runs.iter().any(|r| r.lower.0 == op);
        let bounds_upper = runs.iter().any(|r| r.upper.0 == op);
        let holed = recs.iter().any(|r| r.hole_ops.contains(&op));
        roles[op] = if holed {
            if bounds_lower || bounds_upper {
                return Err(PartFault::SectionNotSimple { op });
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
        domain,
        ends: [lower, upper],
        runs,
        holes,
        roles,
        mu_negative,
        cells,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use develop::extrude::{Apex, Frame};
    use fixtures::devices::cone;
    use geom::content::Edge;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    fn q(n: i128) -> Q {
        Q::from_i128(n)
    }

    /// A disc's boundary as the two x-monotone arcs `arrange2d` decomposes it into.
    fn disc_edges(cx: Q, cy: Q, r: Q) -> Vec<Edge<Bignum>> {
        arrange2d::profile::Profile::new()
            .circle(cx, cy, r)
            .into_edges()
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
        let mut profile = disc_edges(q(0), q(1), Q::new(3, 10));
        profile.extend(disc_edges(q(0), Q::new(5, 2), Q::new(3, 10)));
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
        let extruded = shadow_of(&drilled(disc_edges(q(0), cy.clone(), r.clone())), &q(0));
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
        let mut profile = disc_edges(q(0), q(1), Q::new(1, 500)); // r = 0.002
        profile.extend(disc_edges(q(0), Q::new(5, 2), Q::new(3, 10))); // r = 0.3, 150x wider
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

    /// The axis-aligned square of half-side `h` about `(cx, cy)`.
    fn square_edges(cx: Q, cy: Q, h: Q) -> Vec<Edge<Bignum>> {
        arrange2d::profile::Profile::new()
            .rect(cx, cy, h.clone(), h)
            .into_edges()
    }

    /// **A bounding box has two sides, and both must be bracketed the right way round.** `extent`
    /// is what `bounding_wall` and `reference_point` are built on, and it bracketed segment
    /// endpoints with `rational_above` on *both* sides — a strict upper bound found by doubling
    /// from zero. For this square that gave `[0, 1] × [3, 3]`: not a loose box, a **wrong** one,
    /// with zero height and containing none of the profile. The bounding circle derived from it
    /// missed the square entirely, so the hole window did too, and the multi-wall loop refused a
    /// perfectly good cut. Nothing caught it for two slices because until AUTH.1e.4 no geometry
    /// consumed the box — the polygonal-slot test below only asks that the role is not `Inactive`.
    #[test]
    fn a_polygon_extent_brackets_its_own_corners() {
        let (cx, cy, h) = (q(0), Q::new(11, 5), Q::new(1, 5));
        let Cutter::Extrude(e) = drilled(square_edges(cx.clone(), cy.clone(), h.clone())) else {
            panic!("drilled builds an extrusion")
        };
        let (lo_a, lo_b, hi_a, hi_b) = e.extent().expect("a square has an extent");
        // Tight: within the 2^-48 bisection slop of the true corners, not merely containing them.
        let slack = Q::new(1, 1 << 20);
        for (got, want) in [
            (&lo_a, cx.sub(&h)),
            (&lo_b, cy.sub(&h)),
            (&hi_a, cx.add(&h)),
            (&hi_b, cy.add(&h)),
        ] {
            let d = got.sub(&want);
            let d = if d.sign() < 0 { d.neg() } else { d };
            assert!(
                d.cmp(&slack) != core::cmp::Ordering::Greater,
                "extent bound {} should bracket {} tightly",
                rat_to_f64(got),
                rat_to_f64(&want)
            );
        }
        // And it really contains the profile — the invariant `bounding_wall` rests on.
        assert!(lo_a.cmp(&cx.sub(&h)) != core::cmp::Ordering::Greater);
        assert!(hi_b.cmp(&cy.add(&h)) != core::cmp::Ordering::Less);
    }

    /// the failure `docs/cutter-extrude-design.md` §6 predicted in advance: a square slot subtending
    /// ≈0.045 in σ against ≈0.146 sample cells fell between them, and the resolver derived
    /// `Inactive` — a green certificate on a cut that did nothing.
    ///
    /// The profile's bounding circle supplies the missing window. A superset is the right error:
    /// extra stations sample where the cut is absent and cost nothing, a missing one loses the cut.
    /// **A polygonal slot must be SEEN, however small.** Every wall of a polygon is affine, so it
    /// has no tangent-ruling window — and station targeting keyed on exactly that. The result was
    /// the failure `docs/cutter-extrude-design.md` §6 predicted in advance: a square slot subtending
    /// ≈0.045 in σ against ≈0.146 sample cells fell between them, and the resolver derived
    /// `Inactive` — a green certificate on a cut that did nothing.
    ///
    /// The profile's bounding circle supplies the missing window. A superset is the right error:
    /// extra stations sample where the cut is absent and cost nothing, a missing one loses the cut.
    #[test]
    fn a_polygonal_slot_is_not_dropped_between_sample_cells() {
        use crate::construct;
        use crate::part::SupportFn;
        // A square the same size and place as the disc that already resolves.
        let square = square_edges(q(0), Q::new(11, 5), Q::new(1, 5));

        let part = construct::from_chart::<Bignum>(&cone())
            .region_sigma(Q::new(-7, 2), Q::new(7, 2), SupportFn::inherit())
            .keep_near(
                cone()
                    .surface(&q(2), &q(0))
                    .eval(&q(0))
                    .expect("regular at σ = 0"),
            )
            .intersect(Cutter::half_space([q(0), q(0), q(1)], q(3)))
            .subtract(Cutter::vertical_cylinder(q(0), Q::new(1, 2), q(2)))
            .subtract(Cutter::vertical_cylinder(
                Q::new(-9, 4),
                Q::new(9, 4),
                Q::new(9, 16),
            ))
            .subtract(drilled(square))
            .clearance(q(1));
        let built = part.build_regions().expect("the regions develop");
        let st = sweep(&part, &built).expect("the sweep resolves");
        assert!(
            !matches!(st.roles[3], crate::part::OpRole::Inactive),
            "the slot must be sampled, not dropped between cells — got {:?}",
            st.roles[3]
        );
    }

    /// **The span reaches only as deep as it says.** A transverse drill swept straight through the
    /// cone crosses *both* sheets, so `Through` cuts both regions while `ToNext` cuts only the one
    /// the ray meets first — and the resolver's own hole records show it, not just the reach.
    ///
    /// The ordering is by **ray parameter**: the near sheet is region 1 (σ ≈ +1.08, t ≈ 2.3), the
    /// far one region 0 (σ ≈ −1.08, t ≈ 7.7). Reading depth off σ would pick the wrong sheet.
    #[test]
    fn a_span_cuts_only_the_sheets_it_reaches() {
        use crate::construct;
        use crate::part::SupportFn;
        let mk = |span: Span| {
            construct::from_chart::<Bignum>(&cone())
                .region_sigma(Q::new(-3, 2), q(0), SupportFn::inherit())
                .region_sigma(q(0), Q::new(3, 2), SupportFn::inherit())
                .intersect(Cutter::half_space([q(0), q(0), q(1)], q(3)))
                .subtract(Cutter::vertical_cylinder(q(0), Q::new(1, 2), q(2)))
                .subtract(Cutter::extrude_span(
                    // The sketch plane faces +x at (−5, 0, 3), swept along +x through the cone.
                    Frame::new([q(-5), q(0), q(3)], [q(0), q(1), q(0)], [q(0), q(0), q(1)])
                        .expect("independent axes"),
                    Apex::direction([q(1), q(0), q(0)]).expect("a real direction"),
                    disc_edges(q(0), q(0), Q::new(1, 5)),
                    span,
                ))
                .clearance(q(1))
        };
        let reach_of = |span: Span| {
            let part = mk(span);
            let built = part.build_regions().expect("the regions develop");
            span_reach(&part, &built).expect("the cast resolves")[2].clone()
        };
        assert_eq!(reach_of(Span::Through), None, "Through is unrestricted");
        assert_eq!(
            reach_of(Span::ToNext),
            Some(vec![1]),
            "ToNext reaches the sheet the ray meets first — region 1, at the smaller ray parameter"
        );
        let both = reach_of(Span::NextN(2)).expect("restricted");
        assert_eq!(both.len(), 2, "NextN(2) reaches both sheets");
        assert!(both.contains(&0) && both.contains(&1));

        // And the restriction is visible in what the resolver *derives*, not only in the reach:
        // the drill bounds material in the sheets it reaches and in no others. (Which role it takes
        // — hole or rim notch — depends on where it lands; the span's job is only which sheets.)
        let cut_in = |span: Span| {
            let part = mk(span);
            let built = part.build_regions().expect("the regions develop");
            let st = sweep(&part, &built).expect("the sweep resolves");
            let mut rs: Vec<usize> = Vec::new();
            for run in &st.runs {
                if run.lower.0 != 2 && run.upper.0 != 2 {
                    continue;
                }
                let mid = run.lo.add(&run.hi).mul(&Rat::new(1, 2));
                for (ri, r) in part.regions.iter().enumerate() {
                    if r.band.lo.cmp(&mid) != core::cmp::Ordering::Greater
                        && mid.cmp(&r.band.hi) != core::cmp::Ordering::Greater
                        && !rs.contains(&ri)
                    {
                        rs.push(ri);
                    }
                }
            }
            rs.sort_unstable();
            rs
        };
        assert_eq!(
            cut_in(Span::ToNext),
            vec![1],
            "the shallow cut bounds material in the near sheet only"
        );
        assert_eq!(
            cut_in(Span::Through),
            vec![0, 1],
            "the deep cut bounds material in both"
        );
    }

    /// A ruling that misses the profile entirely is shadowed nowhere — the empty union, which
    /// `subtract` must leave the material untouched by.
    #[test]
    fn a_missed_profile_shadows_nothing() {
        let sh = shadow_of(&drilled(disc_edges(q(9), q(9), Q::new(1, 5))), &q(0));
        assert!(sh.0.is_empty(), "got {:?}", spans(&sh));
    }

    /// The doctest panel over a chosen band, with a witness on the kept sheet — the blank the
    /// AUTH.3 fixture bites into.
    fn blank(lo: Q, hi: Q) -> Part<Bignum> {
        use crate::construct;
        use crate::part::SupportFn;
        construct::from_chart::<Bignum>(&cone())
            .region_sigma(lo, hi, SupportFn::inherit())
            .keep_near(
                cone()
                    .surface(&q(2), &q(0))
                    .eval(&q(0))
                    .expect("the cone is regular at σ = 0"),
            )
            .intersect(Cutter::half_space([q(0), q(0), q(1)], q(3)))
            .subtract(Cutter::vertical_cylinder(q(0), Q::new(1, 2), q(2)))
            .clearance(q(1))
    }

    /// The derived σ-extent and its two ends, or the fault.
    fn extent(part: &Part<Bignum>) -> Result<Structure<Bignum>, PartFault> {
        let built = part.build_regions()?;
        sweep(part, &built)
    }

    /// The merged material components at one σ as `(µ̂_lo, µ̂_hi, op_lo, op_hi)` — the resolver's own
    /// algebra at a single sample, which the sweep's grid need not contain.
    fn comps_at(part: &Part<Bignum>, sigma: &Q) -> Vec<(f64, f64, usize, usize)> {
        let built = part.build_regions().expect("the regions develop");
        let reach = span_reach(part, &built).expect("the cast resolves");
        let zero = q(0);
        let chart = &built.charts[0];
        let mut forms = Vec::with_capacity(part.ops.len());
        for (_, cutter) in part.ops.iter() {
            let walls = cutter.walls().expect("a well-formed cutter");
            forms.push(
                walls
                    .iter()
                    .map(|w| cut_mu_form(chart, w, &zero).expect("a pullback"))
                    .collect::<Vec<_>>(),
            );
        }
        let dj = chart.det_j();
        let flat: Vec<MuCut<Bignum>> = forms.iter().flatten().cloned().collect();
        let regions = vec![RegionForms {
            band: part.regions[0].band.clone(),
            forms,
            flat,
            detj_c: dj.constant,
            detj_m: dj.mu,
        }];
        sample_comps(part, &regions, chart, &reach, 0, sigma)
            .expect("the fixture resolves at every sample")
            .merged
            .iter()
            .map(|m| {
                let (lo, hi) = (m.comp.lo.expect("bounded"), m.comp.hi.expect("bounded"));
                (lo.0, hi.0, lo.1.0, hi.1.0)
            })
            .collect()
    }

    /// **The nearest candidate is not the one that ends the material, and only evaluating between
    /// the brackets can tell.**
    ///
    /// The quadric fixture's end-cell holds two events `2·10⁻³` apart, and this is what happens
    /// across them — measured, not argued. Before `Meet(1, 2)` (σ ≈ −0.2384) the material is bounded
    /// below by the panel's annulus **carve** and above by the contour. After it, **both** bounds
    /// are the contour's own walls, and the interval narrows for another `2·10⁻³` before pinching
    /// shut at `Tangent(2)` (σ ≈ −0.2404). So the `Meet` is a *handover*, not an end.
    ///
    /// A derivation that took the nearest event — or that read the labels at the last live sample
    /// and assumed they persisted — would stop the part at the handover, in the middle of material
    /// the ops leave. This is the assertion that fails for it, and it is the reason the σ-ends are
    /// decided by one evaluation per gap rather than by proximity.
    #[test]
    fn the_nearest_event_in_the_cell_is_not_the_one_that_ends_the_material() {
        let part = blank(Q::new(-7, 2), Q::new(7, 2)).intersect(Cutter::vertical_cylinder(
            q(0),
            Q::new(11, 5),
            q(1),
        ));
        // Before the handover: the carve (op 1) below, the contour (op 2) above.
        let before = comps_at(&part, &Rat::new(-238_400, 1_000_000));
        assert_eq!(before.len(), 1, "one component");
        assert_eq!(
            (before[0].2, before[0].3),
            (1, 2),
            "before the Meet the carve still bounds from below — got {before:?}"
        );
        // Across the stretch between the two candidates: the contour bounds BOTH sides, and the
        // material is still there. Widths taken at the same σ, so they must shrink monotonically.
        let mut last = f64::INFINITY;
        for milli in [-238_500i128, -239_000, -240_000, -240_400] {
            let cs = comps_at(&part, &Rat::new(milli, 1_000_000));
            assert_eq!(
                cs.len(),
                1,
                "σ = {}e-6 must still carry material — the Meet does not end it",
                milli
            );
            assert_eq!(
                (cs[0].2, cs[0].3),
                (2, 2),
                "past the handover both bounds are the contour's own walls — got {cs:?}"
            );
            let width = cs[0].1 - cs[0].0;
            assert!(
                width < last,
                "the interval must be closing: {width:.6} after {last:.6}"
            );
            last = width;
        }
        // And past the tangent it is gone.
        assert!(
            comps_at(&part, &Rat::new(-240_500, 1_000_000)).is_empty(),
            "past Tangent(2) the contour leaves nothing"
        );
    }

    /// σ as a float, for messages.
    fn ff(x: &Q) -> f64 {
        rat_to_f64(x)
    }

    /// **The invariant that lets AUTH.3a land without moving a single pinned number.** Deriving
    /// the σ-extent instead of assuming it may not *change* it where nothing terminates the
    /// material: on a recipe whose ops only trim laterally, the derived domain must be the authored
    /// band exactly — not to a tolerance, the same rationals — and both ends must report
    /// [`SigmaEnd::Band`].
    ///
    /// Asserted structurally rather than by re-running the goldens, because it is the *reason* the
    /// goldens hold: every ε, work counter and chord metric in the repo is measured over a domain
    /// this function now computes rather than reads.
    #[test]
    fn a_laterally_trimmed_recipe_derives_exactly_the_band_it_declared() {
        let part = blank(Q::new(-7, 2), Q::new(7, 2));
        let st = extent(&part).expect("the blank resolves");
        let band = &part.regions[0].band;
        assert!(
            st.domain.lo.cmp(&band.lo) == core::cmp::Ordering::Equal
                && st.domain.hi.cmp(&band.hi) == core::cmp::Ordering::Equal,
            "derived [{:.9}, {:.9}] against the authored [{:.9}, {:.9}]",
            ff(&st.domain.lo),
            ff(&st.domain.hi),
            ff(&band.lo),
            ff(&band.hi)
        );
        assert!(
            matches!(st.ends, [SigmaEnd::Band, SigmaEnd::Band]),
            "a lateral trim ends at the band on both sides"
        );
    }

    /// **The derivation itself** (`docs/cutter-extrude-design.md` §12.2): a contour that terminates
    /// the material derives a strict sub-extent, and each end is an *exactly located event* rather
    /// than a sampled width.
    ///
    /// The two fixtures close by different events — the quadric at its own **tangent ruling**, the
    /// polygon at one of its own **corners** — and the quadric is the one that shows why the choice
    /// among candidates cannot be made by proximity. Its end-cell holds **two** events `2·10⁻³`
    /// apart, and the *nearer* one does not end anything: at `Meet(1, 2)` the lower bound merely
    /// hands over from the panel's annulus carve to the contour's own lower root, after which both
    /// bounds are the contour's walls and the material narrows for another `2·10⁻³` before pinching
    /// at `Tangent(2)`. Measured, at four σ across that stretch — see the `handover` assertion.
    /// Taking the nearest event would have ended the part too early, in the middle of material the
    /// ops leave; only evaluating the component algebra **in the gaps between brackets** decides it,
    /// which is what the isolation buys (#268's recipe one level out).
    ///
    /// The bracket is asserted to be **sharp** — five orders below the sample spacing that found
    /// the end — because "an event lies between the last live sample and the first dead one" is
    /// satisfied by an event set that localizes nothing.
    #[test]
    fn a_contour_that_terminates_the_material_derives_its_ends_exactly() {
        use core::cmp::Ordering;
        let cases: [(&str, Cutter<Bignum>, f64, bool); 2] = [
            (
                "a quadric contour — closes at its own tangent ruling",
                Cutter::vertical_cylinder(q(0), Q::new(11, 5), q(1)),
                0.240_408_206,
                true,
            ),
            (
                "a polygonal contour — closes at its own corner",
                drilled(square_edges(q(0), Q::new(11, 5), Q::new(1, 4))),
                0.063_841_301,
                true,
            ),
        ];
        for (name, cutter, want, want_pinch) in cases {
            let part = blank(Q::new(-7, 2), Q::new(7, 2)).intersect(cutter);
            let st = extent(&part).expect("the contour resolves");
            let band = &part.regions[0].band;
            assert!(
                st.domain.lo.cmp(&band.lo) == Ordering::Greater
                    && st.domain.hi.cmp(&band.hi) == Ordering::Less,
                "{name}: the extent must be a strict sub-interval — got [{:.9}, {:.9}]",
                ff(&st.domain.lo),
                ff(&st.domain.hi)
            );
            // The fixtures are mirror-symmetric, so both ends must land on ∓ the same σ.
            for (end, sign) in st.ends.iter().zip([-1.0, 1.0]) {
                let SigmaEnd::Closed { at, kinds, pinch } = end else {
                    panic!("{name}: both ends close inside the band")
                };
                assert!(
                    (ff(&at.lo) - sign * want).abs() < 1e-8,
                    "{name}: end bracketed at [{:.9}, {:.9}], expected ≈{:+.9} — {kinds:?}",
                    ff(&at.lo),
                    ff(&at.hi),
                    sign * want
                );
                // Sharp: `2⁻⁴⁰ ≈ 9·10⁻¹³` against the 48-cell grid's `7/48 ≈ 0.146`.
                assert!(
                    ff(&at.hi) - ff(&at.lo) < 1e-8,
                    "{name}: the bracket is {:.3e} wide — an event that only localizes to the \
                     sample grid localizes nothing",
                    ff(&at.hi) - ff(&at.lo)
                );
                assert_eq!(
                    *pinch, want_pinch,
                    "{name}: the rails converge here, so the end is a pinch — kinds {kinds:?}"
                );
                // Named by the contour's own wall. Ops 0 and 1 contribute one wall each, so the
                // contour's are indices 2 and up in the union.
                assert!(
                    kinds.iter().any(|k| match k {
                        EventKind::Tangent(i) | EventKind::Escape(i) => *i >= 2,
                        EventKind::Meet(i, j) => *i >= 2 || *j >= 2,
                    }),
                    "{name}: the end must name the contour that made it — got {kinds:?}"
                );
            }
            // And the domain stops *inside* the brackets, never past them: the realized part is
            // short of the true end by at most one bracket width, on the side that keeps the rails
            // real.
            let [
                SigmaEnd::Closed { at: lo, .. },
                SigmaEnd::Closed { at: hi, .. },
            ] = &st.ends
            else {
                unreachable!("checked above")
            };
            assert!(
                st.domain.lo.cmp(&lo.hi) != Ordering::Greater
                    && st.domain.lo.cmp(&lo.lo) != Ordering::Less
                    && st.domain.hi.cmp(&hi.lo) != Ordering::Greater
                    && st.domain.hi.cmp(&hi.hi) != Ordering::Greater,
                "{name}: the domain ends must sit on the inner edge of their own brackets"
            );
        }
    }

    /// **A part is one connected piece, and a wrapping chart is where that stops being obvious.**
    ///
    /// `cone_wrap` covers more than one full turn, so a cutter placed off the axis is met by the
    /// chart **twice** — the same solid contour, two disjoint σ-runs of material. Deriving an
    /// extent across the gap would weld two pieces of the cone into one part through material the
    /// ops removed; picking one silently discards the other. Refused by name instead.
    ///
    /// Not a contrived fixture: this is the self-lapping device's own geometry, and the cutter sits
    /// where its seam drill does.
    #[test]
    fn material_in_two_sigma_runs_is_refused_by_name() {
        use crate::construct;
        use crate::part::SupportFn;
        use fixtures::devices::cone_wrap;
        let rz0 = cone_wrap()
            .ruling()
            .comp(2)
            .eval(&q(0))
            .expect("the wrap chart's ruling is regular at σ = 0");
        let witness = cone_wrap()
            .surface(&q(-3).div(&rz0), &q(0))
            .eval(&q(0))
            .expect("the mid-annulus witness is regular");
        let part = construct::from_chart::<Bignum>(&cone_wrap())
            .region_sigma(Q::new(-5, 4), Q::new(5, 4), SupportFn::constant(q(0)))
            .keep_near(witness)
            .intersect(Cutter::vertical_cylinder(q(0), q(0), Q::new(471, 50)))
            .subtract(Cutter::vertical_cylinder(q(0), Q::new(1, 2), q(4)))
            .clearance(q(1))
            .intersect(Cutter::vertical_cylinder(
                Q::new(-1, 2),
                Q::new(27, 10),
                q(1),
            ));
        assert!(
            matches!(extent(&part), Err(PartFault::DisconnectedRegion)),
            "one contour met twice by a wrapping chart must refuse, not weld or choose"
        );
    }

    // ── BL.1: the pre-state the component refactor must reproduce ────────────────────────────
    //
    // `sweep` collapses every µ̂-component at a σ down to one (`choose_comps`) and records the
    // survivor's two bounding labels in a `Run`. BL.1 keeps them all and *derives* this, so what
    // must not move is exactly what a consumer can see: the derived extent, how it ends, the run
    // partition with its labels, the hole records and the roles. Recorded here **before** the
    // refactor, on the shipped code, so it is a pre-state rather than a restatement of the new
    // behaviour — and paired with `the_digest_moves_when_the_part_does`, because an equality
    // asserted against oneself passes forever.

    /// Everything a consumer reads off a resolved `Structure`, as one line-per-fact string.
    fn digest(st: &Structure<Bignum>) -> String {
        let end = |e: &SigmaEnd<Bignum>| match e {
            SigmaEnd::Band => "Band".to_string(),
            SigmaEnd::Closed { at, kinds, pinch } => format!(
                "Closed[{:.6}, {:.6}] {kinds:?} pinch={pinch}",
                rat_to_f64(&at.lo),
                rat_to_f64(&at.hi)
            ),
        };
        let mut out = format!(
            "domain [{:.6}, {:.6}]\nends {} | {}\n",
            rat_to_f64(&st.domain.lo),
            rat_to_f64(&st.domain.hi),
            end(&st.ends[0]),
            end(&st.ends[1])
        );
        for r in &st.runs {
            out += &format!(
                "run [{:.6}, {:.6}] lower {}/{:?} upper {}/{:?}\n",
                rat_to_f64(&r.lo),
                rat_to_f64(&r.hi),
                r.lower.0,
                r.lower.1,
                r.upper.0,
                r.upper.1
            );
        }
        for (op, ri, w) in &st.holes {
            out += &format!(
                "hole op{op} region{ri} [{:.6}, {:.6}]\n",
                rat_to_f64(&w.lo),
                rat_to_f64(&w.hi)
            );
        }
        for (i, role) in st.roles.iter().enumerate() {
            out += &format!("role op{i} {role:?}\n");
        }
        out += &format!("mu_negative {:?}\n", st.mu_negative);
        out
    }

    /// The banded blank and the same blank with a drill through it — one shape whose boundary is
    /// two rails end to end, one that also carries an interior hole.
    fn pre_state_fixtures() -> Vec<(&'static str, Part<Bignum>)> {
        let band = || blank(Q::new(-7, 2), Q::new(7, 2));
        vec![
            ("banded", band()),
            (
                "notched",
                band().subtract(drilled(disc_edges(
                    Q::new(-9, 4),
                    Q::new(9, 4),
                    Q::new(3, 4),
                ))),
            ),
            (
                // The `r = 1/5` disc at `(0, 11/5)` — the drill the AUTH.1e differential uses, and
                // the one placement here that resolves to an interior `Hole` rather than a `Notch`.
                "holed",
                band().subtract(drilled(disc_edges(q(0), Q::new(11, 5), Q::new(1, 5)))),
            ),
        ]
    }

    #[test]
    fn the_resolved_structure_is_pinned_before_the_component_refactor() {
        let mut got = String::new();
        for (name, part) in pre_state_fixtures() {
            got += &format!("== {name}\n");
            got += &digest(&extent(&part).expect("the fixture resolves"));
        }
        assert_eq!(got, PRE_STATE, "\n--- got ---\n{got}");
    }

    /// The control that makes the pin above non-vacuous: move one op by a hair and the digest must
    /// move with it. Without this, BL.1 could delete the structure and still "reproduce" it.
    #[test]
    fn the_digest_moves_when_the_part_does() {
        let base = blank(Q::new(-7, 2), Q::new(7, 2));
        let moved = blank(Q::new(-7, 2), Q::new(27, 8));
        assert_ne!(
            digest(&extent(&base).expect("resolves")),
            digest(&extent(&moved).expect("resolves")),
            "a part whose band moved by a cell must not digest the same"
        );
    }

    /// The three shapes above as the **shipped** resolver leaves them: a boundary of two rails
    /// end to end, one with a rim bite (`Notch`), one with an interior `Hole`. Every role the
    /// model has, so BL.1 cannot reproduce the pin by covering only the easy one.
    const PRE_STATE: &str = "\
== banded
domain [-3.500000, 3.500000]
ends Band | Band
run [-3.427083, 3.427083] lower 1/Upper upper 0/Plane
role op0 UpperBound
role op1 LowerBound
mu_negative Some(false)
== notched
domain [-3.500000, 3.500000]
ends Band | Band
run [-3.427083, 0.218750] lower 1/Upper upper 0/Plane
run [0.350971, 0.510417] lower 1/Upper upper 2/Wall(0, false)
run [0.656250, 3.427083] lower 1/Upper upper 0/Plane
role op0 UpperBound
role op1 LowerBound
role op2 Notch
mu_negative Some(false)
== holed
domain [-3.500000, 3.500000]
ends Band | Band
run [-3.427083, 3.427083] lower 1/Upper upper 0/Plane
hole op2 region0 [-0.045549, 0.045549]
role op0 UpperBound
role op1 LowerBound
role op2 Hole
mu_negative Some(false)
";

    /// **BL.1's own claim: the one-interval view is a projection of what is now kept.** Every merged
    /// component begins and ends on some raw component's own end, and there are never fewer raw
    /// components than merged ones — the merge only ever folds, never invents.
    ///
    /// Non-vacuous by the second half: the holed fixture must exhibit a σ where the raw list is
    /// *longer* than the merged one. Without that, `raw` could be a copy of `merged` and the slice
    /// would have kept nothing at all — which is exactly how a refactor gated on "nothing changed"
    /// passes while doing nothing.
    #[test]
    fn the_merged_view_is_a_projection_of_the_components_kept() {
        let mut saw_a_fold = false;
        for (name, part) in pre_state_fixtures() {
            let st = extent(&part).expect("the fixture resolves");
            assert!(!st.cells.is_empty(), "{name}: no cells recorded");
            for cell in &st.cells {
                assert!(
                    cell.raw.len() >= cell.merged.len(),
                    "{name} at σ {:.6}: {} raw < {} merged — the merge invented material",
                    rat_to_f64(&cell.sigma),
                    cell.raw.len(),
                    cell.merged.len()
                );
                assert!(cell.pick < cell.merged.len(), "{name}: pick out of range");
                for m in &cell.merged {
                    let (lo, hi) = (m.comp.lo.expect("bounded"), m.comp.hi.expect("bounded"));
                    assert!(
                        cell.raw.iter().any(|r| r.lo.expect("bounded").0 == lo.0),
                        "{name} at σ {:.6}: merged lo {:.6} is on no component",
                        rat_to_f64(&cell.sigma),
                        lo.0
                    );
                    assert!(
                        cell.raw.iter().any(|r| r.hi.expect("bounded").0 == hi.0),
                        "{name} at σ {:.6}: merged hi {:.6} is on no component",
                        rat_to_f64(&cell.sigma),
                        hi.0
                    );
                }
                saw_a_fold |= cell.raw.len() > cell.merged.len();
            }
        }
        assert!(
            saw_a_fold,
            "no sample folded two components into one — `raw` is carrying nothing the merged view \
             does not already have, so BL.1 kept nothing"
        );
    }
}
