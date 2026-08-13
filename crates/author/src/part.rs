//! `part` — the declarative [`Part`] recipe and its evaluation surface.
//!
//! A `Part` **records authoring intent as exact data** (regions, material ops, holes, picks,
//! config); nothing is computed until an evaluator runs. Builder methods are total and
//! infallible (D1): all validation — region tiling, cutter resolution, rail certification —
//! reports as a typed [`PartFault`] inside the one [`Verdict`] each evaluator returns.
//!
//! The facade speaks **product coordinates** (3-D points, azimuth degrees, physical thickness);
//! the core speaks chart coordinates `(σ, µ̂)`. Approximate inputs (azimuth degrees) are snapped
//! to exact rationals and echoed back in the [`ResolveReport`]; recipes are exact end to end.
//!
//! ```
//! use author::part::{Cutter, SupportFn};
//! use author::construct;
//! use certify_core::Verdict;
//! use fixtures::devices::cone;
//! use lattice::{Bignum, Rat};
//!
//! type Q = Rat<Bignum>;
//! // The Stage-1 flex panel, declaratively: a cone gore bounded below the z=3 parallel,
//! // an eccentric inner cylinder carving the annulus, and an interior drill — roles are
//! // DERIVED (bound / hole), never authored.
//! let part = construct::from_chart::<Bignum>(&cone())
//!     .region_sigma(Q::from_i128(-1), Q::from_i128(1), SupportFn::inherit())
//!     .intersect(Cutter::half_space([Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)], Q::from_i128(3)))
//!     .subtract(Cutter::vertical_cylinder(Q::from_i128(0), Q::new(1, 2), Q::from_i128(2)))
//!     .subtract(Cutter::vertical_cylinder(Q::from_i128(0), Q::new(11, 5), Q::new(1, 25)));
//! let flat = match part.develop() {
//!     Verdict::Verified(f) => f,
//!     other => panic!("panel did not certify"),
//! };
//! assert_eq!(flat.region().faces.len(), 1);
//! assert_eq!(flat.region().faces[0].holes.len(), 1); // the drill — derived, not declared
//! ```

use crate::realize;
use crate::resolve;
use certify_core::Verdict;
use develop::cone::{ConeDevelopment, DevConfig};
use develop::cut::CutSurface;
use develop::part::PiecewiseDevelopment;
use develop::unroll::FlatOutline;
use export::trim::RailFit;
use geom::chart::Chart;
use lattice::{Backend, Bignum, Interval, Poly, Rat, RatFunc};

/// A **solid cutter** — a region of space with an unambiguous inside, used by the material ops
/// [`subtract`](Part::subtract)/[`intersect`](Part::intersect). Because cutters are solids (not
/// bare surfaces), the branch/side choice is *derived from containment*, never authored; the
/// planned `Extrude`/`Cone`/`Sphere`/`Quadric` variants join by the same µ̂-pullback class.
pub enum Cutter<B: Backend = Bignum> {
    /// The half-space `{ n·X ≤ d }` (`n` points **out** of the material kept by `intersect`).
    HalfSpace {
        /// The outward plane normal (need not be unit).
        n: [Rat<B>; 3],
        /// The plane offset.
        d: Rat<B>,
    },
    /// The solid cylinder of squared radius `r2` about the axis through `axis_point` along
    /// `axis_dir` (need not be unit).
    Cylinder {
        /// A point on the axis.
        axis_point: [Rat<B>; 3],
        /// The axis direction (nonzero).
        axis_dir: [Rat<B>; 3],
        /// The squared radius `R²` (positive).
        r2: Rat<B>,
    },
}

impl<B: Backend> Cutter<B> {
    /// The half-space `{ n·X ≤ d }`.
    pub fn half_space(n: [Rat<B>; 3], d: Rat<B>) -> Self {
        Cutter::HalfSpace { n, d }
    }

    /// A solid cylinder about an arbitrary axis.
    pub fn cylinder(axis_point: [Rat<B>; 3], axis_dir: [Rat<B>; 3], r2: Rat<B>) -> Self {
        Cutter::Cylinder {
            axis_point,
            axis_dir,
            r2,
        }
    }

    /// The vertical (z-axis-parallel) cylinder over the xy-disk `(cx, cy, r²)` — the flex-PCB
    /// trim idiom (disks drawn in the physical xy-plane).
    pub fn vertical_cylinder(cx: Rat<B>, cy: Rat<B>, r2: Rat<B>) -> Self {
        Cutter::Cylinder {
            axis_point: [cx, cy, Rat::from_i128(0)],
            axis_dir: [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(1)],
            r2,
        }
    }

    /// The cutter's boundary as the engine's [`CutSurface`] (the realization currency).
    pub(crate) fn surface(&self) -> CutSurface<B> {
        match self {
            Cutter::HalfSpace { n, d } => CutSurface::Plane {
                n: [n[0].clone(), n[1].clone(), n[2].clone()],
                d: d.clone(),
            },
            Cutter::Cylinder {
                axis_point,
                axis_dir,
                r2,
            } => CutSurface::Cylinder {
                axis_point: [
                    axis_point[0].clone(),
                    axis_point[1].clone(),
                    axis_point[2].clone(),
                ],
                axis_dir: [
                    axis_dir[0].clone(),
                    axis_dir[1].clone(),
                    axis_dir[2].clone(),
                ],
                r2: r2.clone(),
            },
        }
    }
}

/// Whether a material op removes or keeps the cutter's inside.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpKind {
    /// Remove the cutter's inside from the part.
    Subtract,
    /// Keep only the cutter's inside ("restrict material").
    Intersect,
}

/// A region's support `h`, authored over the region's **unit coordinate** `u ∈ [0, 1]` (mapped
/// σ-affinely onto the snapped band — the only exact option; the support *shape* is approximate
/// design intent anyway). The facade user never writes an `h(σ)`.
pub enum SupportFn<B: Backend = Bignum> {
    /// Constant support `h ≡ c` (`0` = the base sheet).
    Constant(Rat<B>),
    /// The linear ramp `h(0) = h0 → h(1) = h1`.
    Ramp(Rat<B>, Rat<B>),
    /// The cubic smoothstep `h0 → h1` (`h′ = 0` at both ends — the C¹ §8 ramp; gap-free joins
    /// against constant neighbors).
    Smoothstep(Rat<B>, Rat<B>),
    /// An arbitrary rational function of `u` (the escape hatch).
    InU(RatFunc<B>),
    /// Inherit the source chart's own support over this band (the [`from_chart`] idiom).
    ///
    /// [`from_chart`]: crate::construct::from_chart
    Inherit,
}

impl<B: Backend> SupportFn<B> {
    /// Constant support (sugar).
    pub fn constant(h: Rat<B>) -> Self {
        SupportFn::Constant(h)
    }
    /// Linear ramp (sugar).
    pub fn ramp(h0: Rat<B>, h1: Rat<B>) -> Self {
        SupportFn::Ramp(h0, h1)
    }
    /// Cubic smoothstep (sugar).
    pub fn smoothstep(h0: Rat<B>, h1: Rat<B>) -> Self {
        SupportFn::Smoothstep(h0, h1)
    }
    /// Inherit the source chart's support (sugar).
    pub fn inherit() -> Self {
        SupportFn::Inherit
    }

    /// The support as an exact `h(σ)` over `band` (`u = (σ − lo)/(hi − lo)`); `base` is the
    /// source chart's own support (the `Inherit` target).
    pub(crate) fn over(&self, band: &Interval<B>, base: &RatFunc<B>) -> RatFunc<B> {
        let inv = Rat::from_i128(1).div(&band.hi.sub(&band.lo));
        // u(σ) = (σ − lo)/(hi − lo), a degree-1 polynomial.
        let u = Poly::from_coeffs(vec![band.lo.neg().mul(&inv), inv.clone()]);
        match self {
            SupportFn::Constant(h) => RatFunc::from_poly(Poly::constant(h.clone())),
            SupportFn::Ramp(h0, h1) => {
                // h0 + (h1 − h0)·u
                RatFunc::from_poly(Poly::constant(h0.clone()).add(&u.scale(&h1.sub(h0))))
            }
            SupportFn::Smoothstep(h0, h1) => {
                // h0 + (h1 − h0)·(3u² − 2u³)
                let u2 = u.mul(&u);
                let u3 = u2.mul(&u);
                let s = u2
                    .scale(&Rat::from_i128(3))
                    .sub(&u3.scale(&Rat::from_i128(2)));
                RatFunc::from_poly(Poly::constant(h0.clone()).add(&s.scale(&h1.sub(h0))))
            }
            SupportFn::InU(f) => {
                // Compose num/den with the affine u(σ) by Horner.
                let comp = |p: &Poly<B>| -> Poly<B> {
                    let cs = p.coeffs();
                    let mut acc = Poly::zero();
                    for c in cs.iter().rev() {
                        acc = acc.mul(&u).add(&Poly::constant(c.clone()));
                    }
                    acc
                };
                RatFunc::new(comp(f.num()), comp(f.den()))
            }
            SupportFn::Inherit => base.clone(),
        }
    }
}

/// An exact designation resolving a genuinely ambiguous region choice (the witness doctrine's
/// pick vocabulary — extensible; a rational first-hit ray joins later).
pub enum RegionPick<B: Backend = Bignum> {
    /// Keep the material component nearest this 3-D witness point.
    KeepNear([Rat<B>; 3]),
}

/// Why an evaluator refused the recipe — every bare `None`/panic of the demo era, typed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PartFault {
    /// A region's chart is not a recognized developable (the per-region
    /// [`ConeDevelopment`] constructors refused it); carries the region index.
    NotDevelopable(usize),
    /// The recipe declares no regions — a part needs an authored σ-domain.
    NoRegions,
    /// Consecutive region bands leave a gap (index of the later region).
    RegionGap(usize),
    /// Consecutive region bands overlap, or a band is reversed (index of the offender).
    RegionOverlap(usize),
    /// The regions do not share one development frame (angle coefficient + ρ²).
    FrameMismatch,
    /// The material vanished somewhere in the declared σ-domain (the declared regions exceed
    /// the material the ops leave).
    EmptyRegion,
    /// The ops leave no **bounded** material anywhere — the stock discipline: µ̂ is unbounded
    /// until an op bounds it.
    UnboundedRegion,
    /// The resolver could not soundly attribute the material structure at some σ to this op —
    /// genuinely ambiguous (disconnected material) or beyond the resolver's discrimination.
    /// Add a [`keep_near`](Part::keep_near) pick, or simplify the op.
    AmbiguousRegion {
        /// The op whose shadow the resolver could not place.
        op: usize,
    },
    /// A rail of this op did not certify under the clearance (fail-closed; also covers a hole
    /// whose σ-extent could not be bracketed).
    CutUnresolved {
        /// The offending op.
        op: usize,
    },
    /// A hole op's σ-extent crosses a region join (not yet realizable — split the hole or move
    /// the join).
    HoleCrossesRegions {
        /// The offending op.
        op: usize,
    },
    /// The exact flat boolean disagreed with the resolved structure (faces/holes counts) — the
    /// realization is refused rather than shipped inconsistent.
    TopologyMismatch {
        /// Interior cuts the resolution expected.
        expected_holes: usize,
        /// Faces the exact assembly produced.
        faces: usize,
        /// Holes the exact assembly produced.
        holes: usize,
    },
    /// A chart field or rail had a pole in the evaluated range.
    Pole,
    /// The assembled boundary loop failed the unroll's exact chaining check — an internal
    /// realization invariant, refused rather than shipped.
    LoopBroken,
}

/// One declared σ-region: the (snapped) band, its support recipe, and the requested azimuth
/// degrees when it came through [`region_azimuth`](Part::region_azimuth) (for the report echo).
pub(crate) struct RegionSpec<B: Backend> {
    pub band: Interval<B>,
    pub support: SupportFn<B>,
    pub requested_deg: Option<(f64, f64)>,
}

/// The declarative part recipe — regions (piecewise support on one frame) plus material ops,
/// holes, picks, and config, all exact data. Build with [`construct`](crate::construct)'s free
/// functions; evaluate with [`develop`](Part::develop) (flat) or `solid` (STEP).
pub struct Part<B: Backend = Bignum> {
    pub(crate) q: [Poly<B>; 4],
    pub(crate) base_support: RatFunc<B>,
    pub(crate) regions: Vec<RegionSpec<B>>,
    pub(crate) ops: Vec<(OpKind, Cutter<B>)>,
    pub(crate) domain_holes: Vec<Vec<(Rat<B>, Rat<B>)>>,
    pub(crate) pick: Option<RegionPick<B>>,
    pub(crate) clearance: Rat<B>,
    pub(crate) thickness: Rat<B>,
    pub(crate) cfg: DevConfig<B>,
    pub(crate) fit: RailFit,
    pub(crate) segments: usize,
    pub(crate) support_panels: usize,
}

impl<B: Backend> Part<B> {
    /// The bare recipe over a chart frame (the [`construct`](crate::construct) entry points call
    /// this; not part of the public surface).
    pub(crate) fn from_frame(q: [Poly<B>; 4], base_support: RatFunc<B>) -> Self {
        Part {
            q,
            base_support,
            regions: Vec::new(),
            ops: Vec::new(),
            domain_holes: Vec::new(),
            pick: None,
            clearance: Rat::from_i128(1),
            thickness: Rat::new(1, 8),
            cfg: DevConfig::tight(),
            fit: RailFit::default(),
            segments: 48,
            support_panels: 24,
        }
    }

    // — regions (product coordinates first; σ escape hatch kept) —

    /// Declare a region by **azimuth degrees** (the product coordinate): the exact Stage-1 law
    /// `φ = 2·arctan σ` maps degrees to the chart parameter, and each end is **snapped to a
    /// nearby exact rational σ** (recorded, echoed in the [`ResolveReport`]). Successive regions
    /// sharing an endpoint degree snap to the *same* rational, so the bands tile exactly.
    /// (On a wrapped chart the parameter is the chart's own double-cover angle — power users
    /// author in σ via [`region_sigma`](Part::region_sigma).)
    pub fn region_azimuth(mut self, deg: core::ops::Range<f64>, support: SupportFn<B>) -> Self {
        let snap = |d: f64| -> Rat<B> {
            export::approx::f64_to_rat::<B>((d.to_radians() / 2.0).tan(), 30)
        };
        let band = Interval {
            lo: snap(deg.start),
            hi: snap(deg.end),
        };
        self.regions.push(RegionSpec {
            band,
            support,
            requested_deg: Some((deg.start, deg.end)),
        });
        self
    }

    /// Declare a region by an exact σ-band (the power-user escape hatch; exact, no snap).
    pub fn region_sigma(mut self, lo: Rat<B>, hi: Rat<B>, support: SupportFn<B>) -> Self {
        self.regions.push(RegionSpec {
            band: Interval { lo, hi },
            support,
            requested_deg: None,
        });
        self
    }

    // — material ops —

    /// Remove the cutter's inside from the part. The op's **role is derived** at evaluate time —
    /// bounding rail, rim notch, or interior hole — from where its shadow lands in the domain.
    pub fn subtract(mut self, c: Cutter<B>) -> Self {
        self.ops.push((OpKind::Subtract, c));
        self
    }

    /// Keep only the cutter's inside (bound the blank — the stock discipline: µ̂ starts
    /// unbounded, and evaluation faults [`PartFault::UnboundedRegion`] if no op bounds it).
    pub fn intersect(mut self, c: Cutter<B>) -> Self {
        self.ops.push((OpKind::Intersect, c));
        self
    }

    /// An interior cut authored directly in the **domain** `(σ, µ̂)` as a polygon loop (exact
    /// power-user data; the 2-D-flat-authored `hole_flat` joins with the fold extension).
    pub fn hole_domain(mut self, poly: Vec<(Rat<B>, Rat<B>)>) -> Self {
        self.domain_holes.push(poly);
        self
    }

    // — picks —

    /// Resolve a genuinely ambiguous material choice with an exact witness pick.
    pub fn keep(mut self, pick: RegionPick<B>) -> Self {
        self.pick = Some(pick);
        self
    }

    /// Keep the material component nearest this 3-D point (sugar for
    /// [`keep`](Part::keep)`(RegionPick::KeepNear(p))`).
    pub fn keep_near(self, p: [Rat<B>; 3]) -> Self {
        self.keep(RegionPick::KeepNear(p))
    }

    // — config (product quantities; expert hatches kept) —

    /// The fab clearance (the DRC gate is `ε < clearance/2` per certified stage).
    pub fn clearance(mut self, c: Rat<B>) -> Self {
        self.clearance = c;
        self
    }

    /// The sheet thickness — the normal-offset window `[0, t]` the solid evaluator extrudes
    /// through (a physical product quantity).
    pub fn thickness(mut self, t: Rat<B>) -> Self {
        self.thickness = t;
        self
    }

    /// Expert hatch: the develop enclosure budget (series terms + `√` bisection).
    pub fn budget(mut self, cfg: DevConfig<B>) -> Self {
        self.cfg = cfg;
        self
    }

    /// Expert hatch: the rail fit/certification knobs.
    pub fn fit(mut self, fit: RailFit) -> Self {
        self.fit = fit;
        self
    }

    /// Chord segments per boundary rail arc (flat-pattern resolution).
    pub fn segments(mut self, n: usize) -> Self {
        self.segments = n.max(1);
        self
    }

    /// Quadrature panels per γ≠0 region (the verified flat-directrix integrator's refinement).
    pub fn support_panels(mut self, n: usize) -> Self {
        self.support_panels = n.max(1);
        self
    }

    // — evaluators —

    /// Certify and develop the part to its flat pattern: validate the regions, **resolve** the
    /// material ops in-domain (roles derived, conclusive-or-fault), fit + certify every boundary
    /// rail, unroll through the connected piecewise development, and stitch the exact flat
    /// boolean — one [`Verdict`], ε = the max certified bound over all stages.
    pub fn develop(&self) -> Verdict<FlatPattern<B>, PartFault, Rat<B>> {
        let built = match self.build_regions() {
            Ok(b) => b,
            Err(f) => return Verdict::Refuted(f),
        };
        let structure = match resolve::sweep(self, &built) {
            Ok(s) => s,
            Err(f) => return Verdict::Refuted(f),
        };
        realize::flat_pattern(self, &built, structure)
    }

    /// Validate and build the per-region charts + developments and the glued piecewise
    /// development (shared by the evaluators).
    pub(crate) fn build_regions(&self) -> Result<BuiltRegions<B>, PartFault> {
        use core::cmp::Ordering;
        if self.regions.is_empty() {
            return Err(PartFault::NoRegions);
        }
        for (i, r) in self.regions.iter().enumerate() {
            if r.band.lo.cmp(&r.band.hi) != Ordering::Less {
                return Err(PartFault::RegionOverlap(i));
            }
        }
        for i in 1..self.regions.len() {
            match self.regions[i - 1].band.hi.cmp(&self.regions[i].band.lo) {
                Ordering::Less => return Err(PartFault::RegionGap(i)),
                Ordering::Greater => return Err(PartFault::RegionOverlap(i)),
                Ordering::Equal => {}
            }
        }
        let mut charts = Vec::with_capacity(self.regions.len());
        for r in &self.regions {
            let h = r.support.over(&r.band, &self.base_support);
            charts.push(Chart::new(self.q.clone(), h));
        }
        let mut devs = Vec::with_capacity(charts.len());
        for (i, chart) in charts.iter().enumerate() {
            let dev = ConeDevelopment::new(chart)
                .or_else(|| ConeDevelopment::new_developable(chart, self.support_panels))
                .ok_or(PartFault::NotDevelopable(i))?;
            devs.push(dev);
        }
        let glued: Vec<(Interval<B>, ConeDevelopment<B>)> = self
            .regions
            .iter()
            .zip(devs.iter())
            .map(|(r, d)| (r.band.clone(), d.clone()))
            .collect();
        let pw = PiecewiseDevelopment::new(glued).ok_or(PartFault::FrameMismatch)?;
        Ok(BuiltRegions { charts, pw })
    }
}

/// The validated per-region builds: one chart per region plus the glued connected development.
pub(crate) struct BuiltRegions<B: Backend> {
    pub charts: Vec<Chart<B>>,
    pub pw: PiecewiseDevelopment<B>,
}

/// A certified flat pattern: the developed outer boundary, the developed interior cuts, the
/// exact assembled region, the max certified ε, and the [`ResolveReport`] echo.
pub struct FlatPattern<B: Backend = Bignum> {
    pub(crate) outline: FlatOutline<B>,
    pub(crate) holes: Vec<FlatOutline<B>>,
    pub(crate) domain_holes: Vec<Vec<[Rat<B>; 2]>>,
    pub(crate) region: arrange2d::boolean::Region<B>,
    pub(crate) eps: Rat<B>,
    pub(crate) report: ResolveReport<B>,
}

impl<B: Backend> FlatPattern<B> {
    /// The developed outer boundary loop.
    pub fn outline(&self) -> &FlatOutline<B> {
        &self.outline
    }
    /// The developed interior cut loops (derived holes).
    pub fn holes(&self) -> &[FlatOutline<B>] {
        &self.holes
    }
    /// The domain-authored hole polygons, developed.
    pub fn domain_hole_polys(&self) -> &[Vec<[Rat<B>; 2]>] {
        &self.domain_holes
    }
    /// The exact assembled flat region (`outer − ⋃ holes` via the certified 2-D boolean).
    pub fn region(&self) -> &arrange2d::boolean::Region<B> {
        &self.region
    }
    /// The max certified bound over rails, unroll edges, and hole loops.
    pub fn eps(&self) -> &Rat<B> {
        &self.eps
    }
    /// The resolution echo: snapped region σ, derived op roles.
    pub fn report(&self) -> &ResolveReport<B> {
        &self.report
    }

    /// The flat pattern as a self-contained SVG (`px` wide; even-odd fill cuts the holes out).
    pub fn svg(&self, px: u32) -> String {
        let polys = export::svg::region_to_polys(&self.region);
        let frame = export::svg::Bounds::of_points(
            polys
                .faces
                .iter()
                .flat_map(|f| f.rings.iter().flatten().copied()),
        );
        export::svg::polys_svg(&polys, &frame, px)
    }
}

/// The resolution echo: what the approximate inputs snapped to, and what role each op resolved
/// to — the D2 doctrine's "recorded as exact data, echoed back".
pub struct ResolveReport<B: Backend = Bignum> {
    /// Per region: the requested azimuth degrees (if authored that way) and the exact snapped
    /// σ-band actually recorded.
    pub regions: Vec<RegionEcho<B>>,
    /// Per op: the derived role.
    pub ops: Vec<OpReport>,
}

/// One region's echo.
pub struct RegionEcho<B: Backend = Bignum> {
    /// The requested azimuth degrees, when the region was authored in degrees.
    pub requested_deg: Option<(f64, f64)>,
    /// The exact recorded σ-band.
    pub band: Interval<B>,
}

/// One op's derived resolution.
pub struct OpReport {
    /// Whether the op subtracts (else intersects).
    pub subtract: bool,
    /// The derived role.
    pub role: OpRole,
}

/// The derived role of one material op (echoed in the report) — the classification the old
/// role-sugar (`cut_outer`/`cut_inner`/`notch`/`hole_*`) forced the author to hand-pick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpRole {
    /// The op's rail bounds the part from below (smaller µ̂) over the whole domain.
    LowerBound,
    /// The op's rail bounds the part from above over the whole domain.
    UpperBound,
    /// The op bites across the boundary over part of the domain (a rim notch).
    Notch,
    /// The op pierces the interior (a through-hole).
    Hole,
    /// The op never touches the kept material.
    Inactive,
}
