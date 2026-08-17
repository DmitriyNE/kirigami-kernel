//! The **lapped cone** — the self-lapping device as a validated parameter set.
//!
//! A strip of sheet wound round a cone through *more* than one full turn, so its two ends overlap.
//! They cannot occupy the same space, so at least one end steps off the base cone along the normal;
//! where the seam sits relative to the base decides whether that costs one ramp or two.
//!
//! # The parameters, and what each one is measured against
//!
//! | | quantity | datum |
//! |---|---|---|
//! | 1 | apex half-angle | a rational direction `(cos β, sin β)` |
//! | 2 | stack thickness `t` | the normal-offset window the solid extrudes through |
//! | 3 | seam gap `g` | face to face, in the fully-offset stretch |
//! | 4 | which end laps on top | [`OnTop`] |
//! | 5 | seam offset `c` | **mid-surface to mid-surface**, from the base sheet |
//! | 6 | ramp start / ramp end / sheet end, per side | azimuth ([`Azimuth`]) |
//! | + | outer (and optional inner) trim radius | `r²` about the cone axis |
//!
//! **Parameter 5's datum is the one worth stating twice.** The solid's thickness window is `[0, t]`,
//! so the chart surface `w = 0` is a *face* of the sheet, not its mid-surface. Measuring the seam
//! centreline mid-to-mid is both what an engineer means — where does the stack sit relative to the
//! rest of the board — and the only datum on which the placement law closes:
//!
//! > `h_upper = c + t/2 + g/2`,  `h_lower = c − t/2 − g/2`
//!
//! The upper sheet then occupies `[h_upper, h_upper + t]` and the lower `[h_lower, h_lower + t]`,
//! leaving exactly `g` between their facing faces with the gap's midpoint at `c + t/2`. At
//! `c = ±(t/2 + g/2)` one of the two `h` is exactly zero — that end never leaves the base cone, and
//! its ramp vanishes. At `c = 0` the seam straddles the base symmetrically and both ends ramp.
//!
//! # What is exact, and what snaps
//!
//! **The apex is exact whenever the direction is Pythagorean.** The chart is
//! [`wrap_cone`](fixtures::devices::wrap_cone)`(a, b)` with `sin β = (a² − b²)/(a² + b²)` — the
//! Pythagorean generator — so `b/a = tan(45° − β/2)`, and a rational `(cos β, sin β)` gives a
//! rational `b/a` by two half-angle steps. The generator's overall scale is *gauge*: the Hopf map is
//! invariant under `q ↦ λq`, so `(234, 104)` and `(9, 4)` are the same surface, with identical
//! `normal`, `ruling` and `pedal`. Only the ratio is geometry.
//!
//! **The azimuths generally do not.** On the wrapping chart `φ = 4·arctan σ`, so `σ = tan(φ/4)` is a
//! *quarter*-angle tangent: a rational direction rationalizes only if it is Pythagorean **and its
//! half-direction is too**. So [`Azimuth::Direction`] snaps to a nearby exact σ and echoes it (the
//! house doctrine for approximate product coordinates), and [`Azimuth::Sigma`] is the exact escape
//! hatch — which is what lets a device authored in σ be reproduced at δ = 0.
//!
//! # The lap is a rational sign test
//!
//! Because `φ = 4·arctan σ`, two azimuths differ by exactly `2π` iff `arctan σ₁ − arctan σ₀ = π/2`,
//! iff `1 + σ₁σ₀ = 0`. So:
//!
//! - the **2π shift is the Möbius map `σ ↦ −1/σ`** — the same involution Stage 2 re-centred the seam
//!   with, arrived at from the other direction;
//! - **a lap exists iff `1 + σ_ccw · σ_cw < 0`**, a sign test over ℚ;
//! - the overlap windows are exactly `[−1/σ_cw, σ_ccw]` and `[σ_cw, −1/σ_ccw]`.
//!
//! Nothing in the validation needs `arctan`, and nothing in it is a tolerance.
//!
//! # The gap, when a ramp runs into the lap
//!
//! [`GapPolicy::Constant`] requires both ramps to finish before the overlap starts, so the gap is
//! exactly `g` across the whole seam — one number, checked by the σ ordering above. That is the
//! clean shape and not what every device does: today's acceptance part deliberately lets its ramp
//! descend *inside* the lap, so the gap closes over part of it.
//!
//! [`GapPolicy::MinDistance`] allows that and asks **BONDED** for the number instead:
//! [`Lapped::seam_clearance`] runs [`develop::bonded::clear_boxes`] over the two overlap windows and
//! returns a *certified* lower bound on the true 3-D minimum distance — sound despite the tangential
//! shift, which is the whole reason CLEAR exists and a same-ruling normal gap will not do. Its scope
//! is stated rather than implied: CLEAR certifies **rails** (fixed `µ, w`), so this samples the band
//! edges and is a check on the sheets, not a proof about the band between them. The full-band
//! version is `develop::bonded`'s own deferred scaling step.

use author::construct;
use author::part::{Cutter, Part, SupportFn};
use certify_core::Verdict;
use develop::bonded::{ClearFault, LapRail, clear_boxes};
use export::approx::{f64_to_rat, rat_to_f64};
use fixtures::devices::wrap_cone;
use geom::chart::Chart;
use lattice::{Bignum, Interval, Rat};

type Q = Rat<Bignum>;

fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// The dyadic precision an [`Azimuth::Direction`] snaps its σ to — the same grid
/// `author::construct::cone` snaps a half-angle on.
const SNAP_BITS: u32 = 30;

/// Which end of the strip is the **outer** sheet where the two overlap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnTop {
    /// The counter-clockwise end — increasing σ, increasing azimuth.
    Ccw,
    /// The clockwise end — decreasing σ.
    Cw,
}

/// How the seam gap is checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GapPolicy {
    /// Both ramps must reach full offset **before** the overlap begins, so the gap is exactly the
    /// authored `g` everywhere in the seam. Refuses [`LapFault::RampInsideLap`] otherwise.
    Constant,
    /// Allow a ramp to descend inside the lap. Nothing is refused for it; the gap simply varies
    /// there, and [`Lapped::seam_clearance`] reports what it actually reaches.
    MinDistance,
}

/// An azimuth on the wrapping chart.
#[derive(Clone, Debug)]
pub enum Azimuth {
    /// An **atan2-style rational direction** with an explicit turn count:
    /// `φ = atan2(y, x) + turns·2π`. The turn count is not decoration — a lapped strip spans more
    /// than `2π`, and a bare direction is only defined modulo it, so the sheet ends past ±180°
    /// have no other spelling.
    ///
    /// `σ = tan(φ/4)` is snapped to a dyadic and echoed. See the module docs for when that is free.
    Direction {
        /// The direction's `x` component.
        x: Q,
        /// The direction's `y` component.
        y: Q,
        /// Whole turns to add to `atan2(y, x)`.
        turns: i32,
    },
    /// The chart parameter σ, exactly — the escape hatch, and the only spelling that can reproduce
    /// an existing σ-authored device at δ = 0.
    Sigma(Q),
}

impl Azimuth {
    /// The chart parameter this azimuth resolves to.
    pub fn sigma(&self) -> Q {
        match self {
            Azimuth::Sigma(s) => s.clone(),
            Azimuth::Direction { x, y, turns } => {
                let phi = rat_to_f64(y).atan2(rat_to_f64(x))
                    + f64::from(*turns) * 2.0 * core::f64::consts::PI;
                f64_to_rat((phi / 4.0).tan(), SNAP_BITS)
            }
        }
    }
}

/// The three azimuths shaping one end of the strip, ordered **outward** from the base cone.
#[derive(Clone, Debug)]
pub struct SideAngles {
    /// Where this end leaves the base cone.
    pub ramp_start: Azimuth,
    /// Where it reaches its full seam offset. Equal to `ramp_start` on a side whose offset is zero.
    pub ramp_end: Azimuth,
    /// Where the sheet terminates.
    pub sheet_end: Azimuth,
}

impl SideAngles {
    /// All three at one azimuth — the degenerate side of a one-ramp seam, whose end never leaves
    /// the base cone and so has no ramp to place.
    pub fn flat(at: Azimuth) -> Self {
        SideAngles {
            ramp_start: at.clone(),
            ramp_end: at.clone(),
            sheet_end: at,
        }
    }
}

/// The lapped-cone recipe.
#[derive(Clone, Debug)]
pub struct LappedCone {
    /// The apex direction `(cos β, sin β)` — a Pythagorean pair is exact, anything else snaps.
    pub apex: (Q, Q),
    /// The PCB stack thickness `t`.
    pub thickness: Q,
    /// The face-to-face gap `g` in the fully-offset stretch of the seam.
    pub gap: Q,
    /// Which end laps on top.
    pub on_top: OnTop,
    /// The seam centreline, **mid-surface to mid-surface** from the base sheet.
    pub seam_offset: Q,
    /// The counter-clockwise end's three azimuths.
    pub ccw: SideAngles,
    /// The clockwise end's three azimuths.
    pub cw: SideAngles,
    /// The outer trim radius **squared**, about the cone axis.
    pub outer_r2: Q,
    /// The inner trim radius squared, if the blank is an annulus. `None` leaves the inner bound to
    /// the caller's own authoring ops.
    pub inner_r2: Option<Q>,
    /// How the gap is checked.
    pub policy: GapPolicy,
    /// The component pick, when the derived one will not do.
    ///
    /// The wrapping chart covers **both** sheets of a double cover, so the recipe has to designate
    /// which one is material rather than leave it to a rule. `None` derives a point mid-annulus at
    /// `σ = 0` on the lower nappe, which is right whenever the trim radii are the whole story. A
    /// caller whose *own* authoring ops move the material — an off-axis inner bound, say — must name
    /// a point itself, because the derived one could land in what those ops remove.
    pub pick: Option<[Q; 3]>,
}

/// Why a [`LappedCone`] is not a device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LapFault {
    /// The apex direction is not a half-angle in `(0°, 90°)` — a degenerate or inverted cone.
    ApexNotACone,
    /// The stack thickness is not positive.
    ThicknessNotPositive,
    /// The seam gap is negative. Zero is allowed: it is the bonded-contact configuration.
    GapNegative,
    /// The trim radii are not `0 < r_inner² < r_outer²`.
    RadiiNotAnAnnulus,
    /// One end's three azimuths do not run outward from the base cone.
    AngleOrder(OnTop),
    /// The two ends' bases cross — the clockwise end starts after the counter-clockwise one.
    SidesCross,
    /// The strip does not wrap past a full turn, so its ends never overlap and there is no seam.
    /// Exactly `1 + σ_ccw · σ_cw ≥ 0`.
    NoLap,
    /// An end's ramp width and its seam offset disagree, in either direction: a zero offset with a
    /// positive-width ramp is a ramp from the base cone to the base cone, and a nonzero offset with
    /// a zero-width ramp is a **step** in the support — a discontinuous surface, not a ramp.
    /// Refused rather than silently repaired, so the recipe cannot misdescribe the part it builds.
    RampOffsetMismatch(OnTop),
    /// [`GapPolicy::Constant`] was asked for and a ramp descends inside the overlap, so the gap is
    /// not the authored one across the whole seam.
    RampInsideLap(OnTop),
}

/// A validated lapped cone: the part, and the derived quantities the recipe did not state.
pub struct Lapped {
    /// The blank, ready for further authoring ops (a drill, a feature, resolution knobs).
    pub part: Part<Bignum>,
    /// The bare chart the part rides — the source for the seam-clearance rails.
    pub chart: Chart<Bignum>,
    /// The counter-clockwise end's support offset, `0` when that end never leaves the base cone.
    pub h_ccw: Q,
    /// The clockwise end's support offset.
    pub h_cw: Q,
    /// The overlap window on the counter-clockwise end: `[−1/σ_cw_end, σ_ccw_end]`.
    pub lap_ccw: Interval<Bignum>,
    /// The overlap window on the clockwise end: `[σ_cw_end, −1/σ_ccw_end]`.
    pub lap_cw: Interval<Bignum>,
    /// Each region's `(band, support)` as authored — the echo of the σ snap.
    pub regions: Vec<(Interval<Bignum>, Q)>,
    /// The recipe this came from.
    pub spec: LappedCone,
}

/// A certified statement about how close the two lapping sheets come.
pub struct SeamClearance {
    /// The certified lower bound on the **squared** 3-D distance, minimised over the sampled rails.
    pub min_dist_sq: Q,
    /// How many rail pairs it was certified on — the scope of the claim, stated rather than implied.
    pub rails: usize,
    /// Subdivision nodes spent.
    pub nodes: usize,
}

impl SeamClearance {
    /// The bound as a distance, for reporting. The certificate is the squared rational.
    pub fn min_dist(&self) -> f64 {
        rat_to_f64(&self.min_dist_sq).max(0.0).sqrt()
    }
}

/// The generator `(a, b)` of the cone whose half-angle is the direction `(c, s)`.
///
/// `sin β = (a² − b²)/(a² + b²)` is the Pythagorean generator, so `b/a = tan(45° − β/2)`; two
/// half-angle steps give `a = r + c + s`, `b = r + c − s` with `r = |(c, s)|`. Rational whenever
/// `(c, s)` is a Pythagorean pair, and snapped through `f64` otherwise — the overall scale being
/// gauge, no reduction is needed either way.
fn apex_generator(c: &Q, s: &Q) -> Option<(Q, Q)> {
    if s.sign() <= 0 || c.sign() <= 0 {
        return None;
    }
    let r2 = c.mul(c).add(&s.mul(s));
    // Exact when `r²` is a rational square — which is precisely the Pythagorean case, and the
    // dyadic snap reproduces it exactly there. Otherwise the nearest dyadic, which still yields an
    // exact *cone*, only not exactly the requested angle.
    let r = f64_to_rat::<Bignum>(rat_to_f64(&r2).sqrt(), SNAP_BITS);
    let a = r.add(c).add(s);
    let b = r.add(c).sub(s);
    (b.sign() > 0 && a > b).then_some((a, b))
}

/// Validate a recipe and build its blank.
///
/// The part carries the chart, the regions, the trim ops and the thickness — the geometry. The
/// resolution knobs (`clearance`, `segments`, `support_panels`, `fit`, `budget`) and any further
/// features are the caller's, added on the returned [`Part`] builder.
pub fn lapped_cone(spec: &LappedCone) -> Result<Lapped, LapFault> {
    let (t, g, c) = (&spec.thickness, &spec.gap, &spec.seam_offset);
    if t.sign() <= 0 {
        return Err(LapFault::ThicknessNotPositive);
    }
    if g.sign() < 0 {
        return Err(LapFault::GapNegative);
    }
    if spec.outer_r2.sign() <= 0 {
        return Err(LapFault::RadiiNotAnAnnulus);
    }
    if let Some(r2) = &spec.inner_r2 {
        if r2.sign() <= 0 || *r2 >= spec.outer_r2 {
            return Err(LapFault::RadiiNotAnAnnulus);
        }
    }
    let (a, b) = apex_generator(&spec.apex.0, &spec.apex.1).ok_or(LapFault::ApexNotACone)?;

    // — the two supports: `h_upper = c + t/2 + g/2`, `h_lower = c − t/2 − g/2`. —
    let half = |v: &Q| v.div(&qi(2));
    let step = half(t).add(&half(g));
    let (h_ccw, h_cw) = match spec.on_top {
        OnTop::Ccw => (c.add(&step), c.sub(&step)),
        OnTop::Cw => (c.sub(&step), c.add(&step)),
    };

    // — the six azimuths, as σ. —
    let s = |a: &Azimuth| a.sigma();
    let (cw_end, cw_re, cw_rs) = (
        s(&spec.cw.sheet_end),
        s(&spec.cw.ramp_end),
        s(&spec.cw.ramp_start),
    );
    let (ccw_rs, ccw_re, ccw_end) = (
        s(&spec.ccw.ramp_start),
        s(&spec.ccw.ramp_end),
        s(&spec.ccw.sheet_end),
    );
    // Outward ordering, per side: σ runs `sheet_end ≤ ramp_end ≤ ramp_start` going in on the CW
    // end and `ramp_start ≤ ramp_end ≤ sheet_end` going out on the CCW one. Only monotonicity is
    // required — a zero-width *plateau* is a legitimate end (the ramp runs right to the sheet edge),
    // and an offset-free side collapses all three onto one azimuth, which is what
    // [`SideAngles::flat`] writes. What must be non-empty is the base band, below.
    if !(cw_end <= cw_re && cw_re <= cw_rs) {
        return Err(LapFault::AngleOrder(OnTop::Cw));
    }
    if !(ccw_rs <= ccw_re && ccw_re <= ccw_end) {
        return Err(LapFault::AngleOrder(OnTop::Ccw));
    }
    if cw_rs >= ccw_rs {
        return Err(LapFault::SidesCross);
    }
    // A ramp's width and its offset must agree both ways round — see `RampOffsetMismatch`.
    if h_cw.is_zero() != (cw_re == cw_rs) {
        return Err(LapFault::RampOffsetMismatch(OnTop::Cw));
    }
    if h_ccw.is_zero() != (ccw_rs == ccw_re) {
        return Err(LapFault::RampOffsetMismatch(OnTop::Ccw));
    }

    // — the lap, as a sign over ℚ: `1 + σ_ccw·σ_cw < 0` (see the module docs). —
    if qi(1).add(&ccw_end.mul(&cw_end)).sign() >= 0 {
        return Err(LapFault::NoLap);
    }
    // The `2π` shift is `σ ↦ −1/σ`, so the two overlap windows are exact.
    let lap_ccw = Interval {
        lo: cw_end.recip().neg(),
        hi: ccw_end.clone(),
    };
    let lap_cw = Interval {
        lo: cw_end.clone(),
        hi: ccw_end.recip().neg(),
    };
    // Only a side that *has* a ramp can run one into the lap. An offset-free end contributes no
    // ramp at all — its azimuths sit inside its own overlap window by construction — so testing it
    // would refuse every one-ramp seam for a ramp it does not have.
    if spec.policy == GapPolicy::Constant {
        if !h_ccw.is_zero() && ccw_re > lap_ccw.lo {
            return Err(LapFault::RampInsideLap(OnTop::Ccw));
        }
        if !h_cw.is_zero() && cw_re < lap_cw.hi {
            return Err(LapFault::RampInsideLap(OnTop::Cw));
        }
    }

    // — the regions, in σ order: plateau, ramp, base, ramp, plateau. A side with no offset
    //   contributes neither a ramp nor a plateau distinct from the base, so its two empty bands
    //   simply do not appear and a one-ramp seam really is three regions, not five.
    //
    //   `Smoothstep` and not `Ramp` for the two climbing bands: it is C¹ with `h′ = 0` at both
    //   ends, so the joins to the constant neighbours are gap-free and the γ-quadrature meets no
    //   kink. A linear ramp would put a crease at each end of the seam.
    let mut bands: Vec<(Interval<Bignum>, SupportFn<Bignum>, Q)> = Vec::with_capacity(5);
    let mut push = |lo: &Q, hi: &Q, s: SupportFn<Bignum>, h: &Q| {
        if lo < hi {
            bands.push((
                Interval {
                    lo: lo.clone(),
                    hi: hi.clone(),
                },
                s,
                h.clone(),
            ));
        }
    };
    push(&cw_end, &cw_re, SupportFn::constant(h_cw.clone()), &h_cw);
    push(
        &cw_re,
        &cw_rs,
        SupportFn::smoothstep(h_cw.clone(), qi(0)),
        &h_cw,
    );
    push(&cw_rs, &ccw_rs, SupportFn::constant(qi(0)), &qi(0));
    push(
        &ccw_rs,
        &ccw_re,
        SupportFn::smoothstep(qi(0), h_ccw.clone()),
        &h_ccw,
    );
    push(
        &ccw_re,
        &ccw_end,
        SupportFn::constant(h_ccw.clone()),
        &h_ccw,
    );

    let chart = wrap_cone(&a, &b);
    let mut part = construct::from_chart::<Bignum>(&chart);
    let mut regions: Vec<(Interval<Bignum>, Q)> = Vec::with_capacity(bands.len());
    for (band, support, h) in bands {
        part = part.region_sigma(band.lo.clone(), band.hi.clone(), support);
        regions.push((band, h));
    }
    let pick = spec
        .pick
        .clone()
        .unwrap_or_else(|| witness(&chart, &spec.outer_r2, spec.inner_r2.as_ref()));
    part = part.keep_near(pick).intersect(Cutter::vertical_cylinder(
        qi(0),
        qi(0),
        spec.outer_r2.clone(),
    ));
    if let Some(r2) = &spec.inner_r2 {
        part = part.subtract(Cutter::vertical_cylinder(qi(0), qi(0), r2.clone()));
    }
    part = part.thickness(t.clone());

    Ok(Lapped {
        part,
        chart,
        h_ccw,
        h_cw,
        lap_ccw,
        lap_cw,
        regions,
        spec: spec.clone(),
    })
}

/// A point on the kept sheet: mid-annulus at `σ = 0`, on the **lower** nappe.
///
/// The wrapping chart covers both sheets of a double cover, so the recipe must designate the
/// component rather than leave it to a rule. The µ is found in floats and then *evaluated exactly*,
/// so the witness is an exact surface point however it was chosen — a pick is a search input, and
/// which point in the component it is does not matter.
fn witness(chart: &Chart<Bignum>, outer_r2: &Q, inner_r2: Option<&Q>) -> [Q; 3] {
    let zero = qi(0);
    let r = chart.ruling();
    let at0 = |k: usize| r.comp(k).eval(&zero).map(|v| rat_to_f64(&v)).unwrap_or(0.0);
    let (rx, ry, rz) = (at0(0), at0(1), at0(2));
    let rho_r = (rx * rx + ry * ry).sqrt();
    let outer = rat_to_f64(outer_r2).max(0.0).sqrt();
    let inner = inner_r2
        .map(|v| rat_to_f64(v).max(0.0).sqrt())
        .unwrap_or(0.0);
    let target = 0.5 * (outer + inner);
    let mut mu = if rho_r > 0.0 { target / rho_r } else { 1.0 };
    if mu * rz > 0.0 {
        mu = -mu; // the lower nappe, which is the sheet the device keeps
    }
    let mu = f64_to_rat::<Bignum>(mu, SNAP_BITS);
    chart
        .surface(&mu, &zero)
        .eval(&zero)
        .expect("a ruling chart is regular at σ = 0")
}

impl Lapped {
    /// **What BONDED says the seam actually clears.**
    ///
    /// Runs [`clear_boxes`] over the two overlap windows — the head's σ-window against the tail's,
    /// which is why `clear` had to grow a box per rail — and returns a certified lower bound on the
    /// true 3-D minimum distance between the facing faces. Sound despite the tangential shift: the
    /// §7 offset-pair correspondence is displaced by the support derivative, so a same-ruling normal
    /// gap is *not* a min-distance and adaptive subdivision of the real distance is.
    ///
    /// **This proves `≥ keep_out`; it does not measure.** The witness is a lower bound, and every
    /// pruned pair cleared `keep_out` by construction, so the number that comes back is never below
    /// it however roomy the seam is. To learn what the gap actually reaches, bracket: a keep-out
    /// under the true minimum verifies, one above it does not, and bisecting between them tightens
    /// as far as a caller cares to pay for. `Unresolved` carries the closest squared distance the
    /// search reached, which is the useful handle on the failing side of that bracket.
    ///
    /// **Scope.** CLEAR certifies rails at fixed `(µ, w)`. This samples the facing faces at the two
    /// band edges — four rail pairs — so it is a check on the sheets, not a proof about the whole
    /// band between them. Whole-band clearance is `develop::bonded`'s own deferred scaling step.
    pub fn seam_clearance(
        &self,
        keep_out: &Q,
        max_nodes: usize,
    ) -> Verdict<SeamClearance, ClearFault, Q> {
        // The facing faces: the lower sheet's `w = t` face against the upper sheet's `w = 0`.
        let (h_lo, h_hi) = match self.spec.on_top {
            OnTop::Ccw => (&self.h_cw, &self.h_ccw),
            OnTop::Cw => (&self.h_ccw, &self.h_cw),
        };
        let (box_lo, box_hi) = match self.spec.on_top {
            OnTop::Ccw => (&self.lap_cw, &self.lap_ccw),
            OnTop::Cw => (&self.lap_ccw, &self.lap_cw),
        };
        let chart_of = |h: &Q| Chart::new(self.chart.quaternion().clone(), konst(h));
        let (c_lo, c_hi) = (chart_of(h_lo), chart_of(h_hi));

        let mut worst: Option<Q> = None;
        let mut nodes = 0usize;
        let mut rails = 0usize;
        for mu in self.band_edges() {
            let lower = LapRail::from_chart(&c_lo, &mu, &self.spec.thickness);
            let upper = LapRail::from_chart(&c_hi, &mu, &qi(0));
            match clear_boxes(&lower, box_lo, &upper, box_hi, keep_out, max_nodes) {
                Verdict::Verified(w) => {
                    nodes += w.nodes;
                    rails += 1;
                    worst = Some(match worst {
                        Some(m) if m <= w.min_dist_sq => m,
                        _ => w.min_dist_sq,
                    });
                }
                Verdict::Unresolved(d2) => return Verdict::Unresolved(d2),
                Verdict::Refuted(f) => return Verdict::Refuted(f),
            }
        }
        match worst {
            Some(min_dist_sq) => Verdict::Verified(SeamClearance {
                min_dist_sq,
                rails,
                nodes,
            }),
            None => Verdict::Refuted(ClearFault::DegenerateBox),
        }
    }

    /// The ruling coordinates of the trim radii at `σ = 0` — the band edges the clearance samples.
    fn band_edges(&self) -> Vec<Q> {
        let zero = qi(0);
        let r = self.chart.ruling();
        let at0 = |k: usize| r.comp(k).eval(&zero).map(|v| rat_to_f64(&v)).unwrap_or(0.0);
        let (rx, ry, rz) = (at0(0), at0(1), at0(2));
        let rho = (rx * rx + ry * ry).sqrt();
        if rho <= 0.0 {
            return vec![qi(1)];
        }
        let sign = if rz > 0.0 { -1.0 } else { 1.0 };
        let mut out = Vec::with_capacity(2);
        let mut push = |r2: &Q| {
            let mu = sign * rat_to_f64(r2).max(0.0).sqrt() / rho;
            out.push(f64_to_rat::<Bignum>(mu, SNAP_BITS));
        };
        push(&self.spec.outer_r2);
        if let Some(r2) = &self.spec.inner_r2 {
            push(r2);
        }
        out
    }
}

/// The constant rational function `h` — a plateau support.
fn konst(h: &Q) -> lattice::RatFunc<Bignum> {
    lattice::RatFunc::from_poly(lattice::Poly::constant(h.clone()))
}
