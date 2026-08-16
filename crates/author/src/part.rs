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
use develop::cone::{ConeDevelopment, DevConfig, FlatBox};
use develop::cut::CutSurface;
use develop::extrude::{Apex, Cast, ExtrudeFault, Frame};
use develop::fold::{FoldFault, FoldedWire, fold_outline_pw};
use develop::part::PiecewiseDevelopment;
use develop::pick::{Ray, Span};
use develop::unroll::FlatOutline;
use export::trim::RailFit;
use geom::chart::Chart;
use geom::content::{Circle, Edge};
use lattice::{Backend, Bignum, Interval, Poly, Rat, RatFunc, Surd};

/// The σ-bisection depth of the facade's fold evaluators (`(4/7)⁶⁰ ≈ 3·10⁻¹⁵` of the piece
/// width — well under any fab-plausible ε floor; the enclosure budget is [`Part::budget`]).
pub(crate) const FOLD_ITERS: usize = 60;

/// Translate an engine [`FoldFault`] into the facade's typed fault.
pub(crate) fn map_fold_fault(f: FoldFault) -> PartFault {
    match f {
        // Unreachable after `build_regions` (every region already developed) — typed anyway.
        FoldFault::NotACone => PartFault::NotDevelopable(0),
        FoldFault::DegenerateDomain => PartFault::EmptyRegion,
        FoldFault::OutOfGore => PartFault::OutOfGore,
        FoldFault::PoleInEval => PartFault::Pole,
        FoldFault::EmptyLoop => PartFault::EmptyFeature,
        // Unreachable: `BuiltRegions` keeps charts parallel to the gluing by construction.
        FoldFault::ChartMismatch => PartFault::FrameMismatch,
        FoldFault::AmbiguousPreimage => PartFault::AmbiguousPreimage,
    }
}

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
    /// A **sketch swept from an apex**: a profile region drawn in a rational frame and extruded,
    /// parallel or drafted (`docs/cutter-extrude-design.md`). Boxed because it is much the largest
    /// variant and the two metric cutters are what the existing pipelines pass around.
    Extrude(Box<Extrusion<B>>),
}

/// The authored data of an extruded cutter, kept exactly as drawn.
///
/// Validation is deliberately **not** here: a `Part` records intent, and the frame/apex pair is
/// checked when the part is built (an apex in the frame plane becomes a
/// [`PartFault`](crate::part::PartFault), never a panic at authoring time).
pub struct Extrusion<B: Backend = Bignum> {
    /// The plane the profile is drawn in.
    pub frame: Frame<B>,
    /// The sweep's apex — a direction (parallel) or a cast point (drafted).
    pub apex: Apex<B>,
    /// The profile's boundary in frame coordinates, as `arrange2d` edges: non-convex profiles and
    /// holes need no decomposition, because the fill rule stays with the region.
    pub profile: Vec<Edge<B>>,
    /// How deep the cut reaches, counted in neutral surfaces along the reference ray.
    pub span: Span,
}

/// A rational **upper** bound on `√r2`, by three Newton steps from `1`. Newton on `t ↦ (t + r2/t)/2`
/// approaches the root from above after the first step, so the result brackets the true radius
/// without ever taking one — which is what lets a profile extent be computed over `r²` alone.
fn rational_sqrt_above<B: Backend>(r2: &Rat<B>) -> Rat<B> {
    let two = Rat::from_i128(2);
    let mut t = Rat::from_i128(1);
    for _ in 0..3 {
        if t.sign() <= 0 {
            return Rat::from_i128(1);
        }
        t = t.add(&r2.div(&t)).div(&two);
    }
    t
}

/// The profile's extent in frame coordinates: `(lo_a, lo_b, hi_a, hi_b)`.
type Extent<B> = (Rat<B>, Rat<B>, Rat<B>, Rat<B>);

/// A **tight** rational bracket `lo ≤ s < hi` around a possibly-algebraic coordinate.
///
/// `rational_below`/`rational_above` bracket by *doubling from zero*, so on their own they answer
/// at integer scale — `1/5` brackets to `[0, 1]` and `2` to `[0, 3]`. That is a correct bracket and
/// a useless box: for a profile a fraction of a unit across, the derived bounding circle comes out
/// an order of magnitude too big. Bisecting brings both ends within `2⁻⁴⁸` of the true value while
/// keeping the containment invariant (`lo` only ever moves to a value `≤ s`, `hi` to one `> s`).
fn bracket<B: Backend>(s: &Surd<B>) -> (Rat<B>, Rat<B>) {
    use core::cmp::Ordering;
    let (mut lo, mut hi) = (
        arrange2d::locate::rational_below(s),
        arrange2d::locate::rational_above(s),
    );
    for _ in 0..48 {
        let m = lo.add(&hi).mul(&Rat::new(1, 2));
        if s.cmp(&Surd::from_rat(m.clone())) == Ordering::Less {
            hi = m;
        } else {
            lo = m;
        }
    }
    (lo, hi)
}

impl<B: Backend> Extrusion<B> {
    /// A rectangle containing the whole profile, in frame coordinates.
    ///
    /// A segment endpoint may be algebraic (`Surd`, after a boolean), so it is [`bracket`]ed rather
    /// than used exactly — an extent only has to *contain*. Both ends of the bracket are needed:
    /// taking `rational_above` for the low side too was AUTH.1f's own bug, and it did not merely
    /// inflate the box, it **collapsed** it — a square with horizontal edges at `b = 2` and
    /// `b = 12/5` came out with `lo_b = hi_b = 3`, a box the profile is nowhere inside.
    pub(crate) fn extent(&self) -> Option<Extent<B>> {
        let mut bbox: Option<Extent<B>> = None;
        let mut grow = |lo_x: Rat<B>, lo_y: Rat<B>, hi_x: Rat<B>, hi_y: Rat<B>| {
            let least = |p: Rat<B>, q: Rat<B>| {
                if p.cmp(&q) == core::cmp::Ordering::Less {
                    p
                } else {
                    q
                }
            };
            let most = |p: Rat<B>, q: Rat<B>| {
                if p.cmp(&q) == core::cmp::Ordering::Greater {
                    p
                } else {
                    q
                }
            };
            bbox = Some(match bbox.take() {
                None => (lo_x, lo_y, hi_x, hi_y),
                Some((a, b, c, d)) => {
                    (least(lo_x, a), least(lo_y, b), most(hi_x, c), most(hi_y, d))
                }
            });
        };
        for e in &self.profile {
            match e {
                Edge::Arc(a) => {
                    let r = rational_sqrt_above(&a.circle.r2);
                    grow(
                        a.circle.cx.sub(&r),
                        a.circle.cy.sub(&r),
                        a.circle.cx.add(&r),
                        a.circle.cy.add(&r),
                    );
                }
                Edge::Seg(sg) => {
                    for p in [&sg.start, &sg.end] {
                        let (lo_x, hi_x) = bracket(&p.x);
                        let (lo_y, hi_y) = bracket(&p.y);
                        grow(lo_x, lo_y, hi_x, hi_y);
                    }
                }
            }
        }
        bbox
    }
}

impl<B: Backend> Extrusion<B> {
    /// The projection this extrusion works through, or the fault its authoring carries.
    pub(crate) fn cast(&self) -> Result<Cast<B>, ExtrudeFault> {
        Cast::new(self.frame.clone(), self.apex.clone())
    }

    /// A point **inside the profile**, in frame coordinates — the "designated profile point" the
    /// span's reference ray runs through (`docs/cutter-extrude-design.md` §5).
    ///
    /// Derived rather than authored, and **searched** rather than computed: a grid over the
    /// profile's own extent, returning the first point its fill rule accepts. `None` if none is
    /// accepted, which a caller turns into a refusal — a span with no interior point to measure
    /// from is not a span.
    ///
    /// Two candidates that look obvious are deliberately not tried first. The **frame origin** need
    /// not lie in the profile at all, and on a cone-charted part it is typically the apex, where the
    /// reference ray runs along a ruling and the cast is rightly refused as ungrounded. A circle's
    /// **centre** is worse than useless: it sits exactly on the row that exact ray-casting excludes
    /// (`arrange2d::locate`'s genericity precondition), so the fill rule cannot answer there at all.
    /// The grid's odd-fraction offsets keep samples off those rows.
    pub(crate) fn reference_point(&self) -> Option<(Rat<B>, Rat<B>)> {
        let cast = self.cast().ok()?;
        let (lo_x, lo_y, hi_x, hi_y) = self.extent()?;
        // **Even** on purpose. The samples sit at `lo + (2j+1)·w/(2K)`, and with an odd `K` the
        // middle one lands exactly on the extent's centre — which for a disc is the circle's own
        // centre row, the one row exact ray-casting excludes. An even `K` can never produce it,
        // since `2j+1` is odd.
        const K: i128 = 10;
        let (wx, wy) = (hi_x.sub(&lo_x), hi_y.sub(&lo_y));
        for i in 0..K {
            for j in 0..K {
                let a = lo_x.add(&wx.mul(&Rat::new(2 * i + 1, 2 * K)));
                let b = lo_y.add(&wy.mul(&Rat::new(2 * j + 1, 2 * K)));
                if cast.contains(&self.frame.point(&a, &b), &self.profile) == Some(true) {
                    return Some((a, b));
                }
            }
        }
        None
    }

    /// A single wall whose σ-window **contains** the whole profile's: the wall of the profile's
    /// bounding circle, cast the same way.
    ///
    /// Station targeting needs the σ-range where a cutter is active, and for a quadric wall that is
    /// its tangent-ruling window. A profile of straight edges has **no** such window — every wall is
    /// affine — so a polygonal slot would receive no targeted stations and be dropped between sample
    /// cells, which is exactly the failure `docs/cutter-extrude-design.md` §6 predicted. Bounding the
    /// profile by a circle restores one window for the whole cutter, and a *superset* is the right
    /// error: extra stations sample where the cut is absent and cost nothing, whereas a missing one
    /// loses the feature silently.
    pub(crate) fn bounding_wall(&self) -> Option<CutSurface<B>> {
        let cast = self.cast().ok()?;
        let (lo_a, lo_b, hi_a, hi_b) = self.extent()?;
        let (cx, cy) = (
            lo_a.add(&hi_a).mul(&Rat::new(1, 2)),
            lo_b.add(&hi_b).mul(&Rat::new(1, 2)),
        );
        // The circumscribing radius², from the half-diagonal.
        let (wa, wb) = (
            hi_a.sub(&lo_a).mul(&Rat::new(1, 2)),
            hi_b.sub(&lo_b).mul(&Rat::new(1, 2)),
        );
        let r2 = wa.mul(&wa).add(&wb.mul(&wb));
        if r2.sign() <= 0 {
            return None;
        }
        cast.circle_wall(&Circle { cx, cy, r2 }).ok()
    }

    /// The **reference ray** the span counts along: the generatrix through
    /// [`reference_point`](Self::reference_point). For a direction apex that point cast along the
    /// direction; for a cast point, the ray from the apex through it.
    pub(crate) fn reference_ray(&self) -> Option<Ray<B>> {
        let (a, b) = self.reference_point()?;
        let p = self.frame.point(&a, &b);
        Some(match self.apex.finite() {
            Some(x) => Ray {
                origin: [x[0].clone(), x[1].clone(), x[2].clone()],
                dir: [p[0].sub(&x[0]), p[1].sub(&x[1]), p[2].sub(&x[2])],
            },
            None => Ray {
                origin: p,
                dir: self.apex.a().clone(),
            },
        })
    }
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

    /// A profile drawn in `frame` and swept from `apex`, cutting **every** surface it reaches.
    ///
    /// The profile is an `arrange2d` boundary in frame coordinates; its own even-odd fill decides
    /// what is inside, so a non-convex outline or one with holes needs no decomposition.
    pub fn extrude(frame: Frame<B>, apex: Apex<B>, profile: Vec<Edge<B>>) -> Self {
        Self::extrude_span(frame, apex, profile, Span::Through)
    }

    /// The same, reaching only as deep as `span` counts along the reference ray.
    ///
    /// The span counts **neutral surfaces** — chart embeddings — because cuts are authored before
    /// any stackup exists, so there are no layers or faces to count yet. See
    /// [`Extrusion::reference_ray`] for the ray it is measured along.
    pub fn extrude_span(frame: Frame<B>, apex: Apex<B>, profile: Vec<Edge<B>>, span: Span) -> Self {
        Cutter::Extrude(Box::new(Extrusion {
            frame,
            apex,
            profile,
            span,
        }))
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

    /// The cutter's boundary as the engine's [`CutSurface`]s — **one per wall**, in a fixed order
    /// a [`Label`](crate::resolve::Label) can index into.
    ///
    /// This replaced a `surface() -> CutSurface`, which could not describe a cutter whose boundary
    /// is several surfaces. The two metric cutters return exactly one wall, so every existing
    /// caller reads `walls()[0]` and nothing about them changed.
    pub(crate) fn walls(&self) -> Result<Vec<CutSurface<B>>, ExtrudeFault> {
        if let Cutter::Extrude(e) = self {
            let cast = e.cast()?;
            // Distinct carriers, not edges: a disc arrives as two arcs of one circle, and a
            // duplicated wall is counted twice by everything downstream that counts walls.
            return cast.carrier_walls(&e.profile);
        }
        Ok(vec![self.metric_surface()])
    }

    /// The single boundary surface of a metric cutter. Panics for an extrusion, which has no
    /// single surface — callers go through [`walls`](Self::walls).
    fn metric_surface(&self) -> CutSurface<B> {
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
            // Unreachable: `walls` routes extrusions before this is called. Typed rather than
            // panicking, so a future variant cannot silently take a wrong surface.
            Cutter::Extrude(_) => CutSurface::Plane {
                n: [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)],
                d: Rat::from_i128(0),
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
    /// The material's derived σ-extent does not fill the declared σ-domain — either the ops leave
    /// nothing anywhere, or they leave material over a strict sub-interval of the declared regions
    /// (a cutter that *terminates* the blank rather than trimming it laterally). The second case is
    /// what AUTH.3b lifts; until then it is refused rather than realized over a domain the material
    /// does not fill (`docs/cutter-extrude-design.md` §12).
    EmptyRegion,
    /// The ops leave no **bounded** material at some σ — the stock discipline: µ̂ is unbounded
    /// until an op bounds it, and an unbounded component is not part material.
    ///
    /// Distinct from [`EmptyRegion`](PartFault::EmptyRegion), and the distinction is load-bearing:
    /// material that is *absent* at a σ puts that σ outside the derived extent, while material that
    /// is *unbounded* there is a stock error. Trimming the extent to where the blank happens to be
    /// bounded would ship a part whose end is the unboundedness frontier rather than a cut.
    UnboundedRegion,
    /// The ops leave material in **more than one** σ-run — two or more pieces with a gap between
    /// them. A part is one connected piece; the resolver refuses rather than picking a run or
    /// emitting several.
    DisconnectedRegion,
    /// A boundary rail is needed over σ its certificate does not cover.
    ///
    /// The fit clamps to the wall's disc-positive window, which was always wider than the outer
    /// boundary needed until a **derived** σ-end could land on a tangent ruling. Evaluating the
    /// fitted graph past its certified span there is extrapolation into a √-branch, so the rail is
    /// refused rather than used — the pinch end needs a p-curve, not a longer graph
    /// (`docs/cutter-extrude-design.md` §12.4).
    RailSpanShort {
        /// The op whose rail runs out of certificate.
        op: usize,
    },
    /// The material's σ-extent ends inside the declared domain, but no structural event brackets
    /// that end, so the resolver cannot locate it exactly.
    ///
    /// Where material stops, the two walls bounding the kept µ̂-interval cross at a common µ̂ (a
    /// `Meet` or `Tangent` event) or a µ̂-degenerate wall flips its coverage (a
    /// [`coverage_events`](develop::cut::coverage_events) root). An end matching none of those is
    /// beyond what the resolver can attribute, and it refuses rather than bisecting on a float
    /// width — a σ-end found by search is a boundary nothing certifies.
    SigmaEndUnattributed,
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
    /// This op's cutter meets some ruling in **more than one stretch** — an extruded profile that
    /// is non-convex, or that has a hole of its own. An interior hole is realized as a *band* (one
    /// lower boundary, one upper), which cannot express that footprint, so the cut is refused
    /// rather than approximated by one of its stretches. Author the feature as several convex
    /// cuts, or wait for holes to be regions end-to-end (`docs/cutter-extrude-design.md` §10.1).
    ProfileNotSimple {
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
    /// The solid builder refused the certified chains (degenerate band, or a hole not strictly
    /// interior / σ-disjoint — the `brep_trim_solid_regions` preconditions).
    SolidRefused,
    /// A flat point handed to the fold (a [`fold`](Part::fold) feature vertex, or a
    /// [`hole_flat`](Part::hole_flat) vertex at solid time) lies outside the part's developed
    /// gore — no σ in the declared domain develops to its direction.
    OutOfGore,
    /// A fold was requested on an empty feature loop, or an authored polygon hole has fewer
    /// than three vertices.
    EmptyFeature,
    /// The fold's µ̂-side convention is undefined: the resolved material does not keep one sign
    /// of µ̂ across the whole domain (it straddles the pedal locus µ̂ = 0, or genuinely uses both
    /// sheets). [`fold`](Part::fold) and [`hole_flat`](Part::hole_flat) need a single-side part.
    SideAmbiguous,
    /// A flat point handed to the fold lies where the development overlaps itself (a part whose
    /// flat sector exceeds 360°): two σ-disjoint preimages both certify, so no sound choice
    /// exists. Author the feature outside the lap wedge.
    AmbiguousPreimage,
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
    pub(crate) flat_holes: Vec<Vec<[Rat<B>; 2]>>,
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
            flat_holes: Vec::new(),
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
    /// power-user data). Like [`hole_flat`](Part::hole_flat) it must stay disjoint from every
    /// other cut — both evaluators gate authored polygons through the exact flat boolean.
    pub fn hole_domain(mut self, poly: Vec<(Rat<B>, Rat<B>)>) -> Self {
        self.domain_holes.push(poly);
        self
    }

    /// An interior cut authored directly in the **flat pattern** (ECAD 2-D coordinates) as a
    /// polygon loop. [`develop`](Part::develop) cuts it into the exact flat boolean as-is (it is
    /// already flat data); [`solid`](Part::solid) **folds it back** onto the surface through the
    /// certified piecewise fold-inversion (the µ̂-side derived from the resolution) and drills it
    /// through the solid. Vertex winding is free; the polygon must stay disjoint from every
    /// other cut (the flat boolean's holes are pairwise disjoint — an overlap breaks the
    /// expected topology and the coherence gate refuses the evaluation, on **both** evaluators:
    /// `solid` runs the same exact flat boolean before building whenever authored polygon holes
    /// are present).
    pub fn hole_flat(mut self, poly: Vec<[Rat<B>; 2]>) -> Self {
        self.flat_holes.push(poly);
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

    /// Certify and build the part's **watertight solid**: the same resolution as
    /// [`develop`](Part::develop), re-certified at the internal low-degree STEP rail profile,
    /// extruded through the configured [`thickness`](Part::thickness) window `[0, t]` and sewn
    /// by the certified curved-rail builder (`brep_trim_solid_regions`) — derived holes become
    /// through-holes, domain-authored polygons become polygon cuts. Write it with
    /// [`PartSolid::write_step`] (behind the `step` feature).
    pub fn solid(&self) -> Verdict<PartSolid<B>, PartFault, Rat<B>> {
        let built = match self.build_regions() {
            Ok(b) => b,
            Err(f) => return Verdict::Refuted(f),
        };
        let structure = match resolve::sweep(self, &built) {
            Ok(s) => s,
            Err(f) => return Verdict::Refuted(f),
        };
        // The authored-polygon coherence gate: domain- and flat-authored holes are validated by
        // the same exact flat boolean [`develop`](Part::develop) runs (containment + pairwise
        // disjointness, surfaced through the face/hole counts). The solid builder's own checks
        // are slice-local — an overlapping or out-of-band polygon must refuse here, never sew a
        // self-intersecting shell.
        if !self.domain_holes.is_empty() || !self.flat_holes.is_empty() {
            match realize::flat_pattern(self, &built, structure.clone()) {
                Verdict::Verified(_) => {}
                Verdict::Unresolved(e) => return Verdict::Unresolved(e),
                Verdict::Refuted(f) => return Verdict::Refuted(f),
            }
        }
        match realize::solid_brep(self, &built, structure) {
            Verdict::Verified((brep, eps, report)) => {
                Verdict::Verified(PartSolid { brep, eps, report })
            }
            Verdict::Unresolved(e) => Verdict::Unresolved(e),
            Verdict::Refuted(f) => Verdict::Refuted(f),
        }
    }

    /// Fold a **flat-authored feature** (a polyline or loop in the certified flat frame — ECAD
    /// coordinates, the same frame [`develop`](Part::develop) emits) back onto the 3-D surface at
    /// normal offset `w`: the certified piecewise fold-inversion, direction ② of the product
    /// round-trip. The µ̂-side is **derived from the resolution** (never authored — a mixed-side
    /// part is refused as [`PartFault::SideAmbiguous`]); each vertex is inverted in whichever
    /// region's running frame brackets it and lifted onto that region's chart. Returns the folded
    /// 3-D wire under the round-trip DRC `ε <` [`clearance`](Part::clearance)`/2`.
    pub fn fold(
        &self,
        feature: &[[Rat<B>; 2]],
        w: &Rat<B>,
    ) -> Verdict<FoldedWire<B>, PartFault, Rat<B>> {
        let built = match self.build_regions() {
            Ok(b) => b,
            Err(f) => return Verdict::Refuted(f),
        };
        let structure = match resolve::sweep(self, &built) {
            Ok(s) => s,
            Err(f) => return Verdict::Refuted(f),
        };
        let side = match structure.mu_negative {
            Some(s) => s,
            None => return Verdict::Refuted(PartFault::SideAmbiguous),
        };
        match fold_outline_pw(
            &built.pw,
            &built.charts,
            feature,
            w,
            FOLD_ITERS,
            side,
            &self.cfg,
            &self.clearance,
        ) {
            Verdict::Verified(wire) => Verdict::Verified(wire),
            Verdict::Unresolved(e) => Verdict::Unresolved(e),
            Verdict::Refuted(f) => Verdict::Refuted(map_fold_fault(f)),
        }
    }

    /// Where the **rulings** at `sigmas` land in the developed flat pattern: for each, the
    /// certified flat images of the domain points `(σ, 0)` and `(σ, 1)` — one point on the image
    /// line and a second fixing its direction.
    ///
    /// Direction ① is an isometry and a ruling is straight, so its flat image is a straight line.
    /// Where the support is constant that line passes through the flat apex, and the whole family
    /// is a pencil of rays from the origin; where the support **curves**, each line is offset by
    /// the running flat directrix `γ(σ)` and the family is no longer concurrent. Reading a ruling
    /// off the pattern therefore takes the development, not trigonometry — which is why this is
    /// here rather than in a consumer.
    ///
    /// Plural because the glued development memoizes its γ-quadrature: asking for one σ at a time
    /// rebuilds and re-integrates it every call.
    ///
    /// Errs [`PartFault::Pole`] if a σ lies outside the declared σ-domain or the directrix has a
    /// pole there; otherwise the region faults of [`develop`](Part::develop).
    pub fn flat_rulings(&self, sigmas: &[Rat<B>]) -> Result<Vec<[FlatBox<B>; 2]>, PartFault> {
        let built = self.build_regions()?;
        let (zero, one) = (Rat::from_i128(0), Rat::from_i128(1));
        sigmas
            .iter()
            .map(|s| {
                let at = |m: &Rat<B>| built.pw.point(s, m, &self.cfg).ok_or(PartFault::Pole);
                Ok([at(&zero)?, at(&one)?])
            })
            .collect()
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
    pub(crate) flat_holes: Vec<Vec<[Rat<B>; 2]>>,
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
    /// The flat-authored hole polygons, as authored (already flat coordinates).
    pub fn flat_hole_polys(&self) -> &[Vec<[Rat<B>; 2]>] {
        &self.flat_holes
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

/// A certified part solid: the exact watertight B-rep (curved rails, lids, walls, drilled
/// holes), the max certified rail bound, and the resolution echo.
pub struct PartSolid<B: Backend = Bignum> {
    pub(crate) brep: export::brep::Brep<B>,
    pub(crate) eps: Rat<B>,
    pub(crate) report: ResolveReport<B>,
}

impl<B: Backend> PartSolid<B> {
    /// The exact boundary representation (shared vertex/edge tables, curved rail Béziers).
    pub fn brep(&self) -> &export::brep::Brep<B> {
        &self.brep
    }
    /// The max certified rail bound of the STEP re-fit.
    pub fn eps(&self) -> &Rat<B> {
        &self.eps
    }
    /// The resolution echo (same shape as the flat evaluator's).
    pub fn report(&self) -> &ResolveReport<B> {
        &self.report
    }

    /// Certify the closed shell, write the `.step` via OCCT, and report both — one call
    /// ([`emit_certified_step`](export::step::emit_certified_step)). Needs the `step` feature
    /// (build under `nix develop`).
    #[cfg(feature = "step")]
    pub fn write_step(&self, path: &str) -> export::step::StepReport {
        export::step::emit_certified_step(path, &self.brep)
    }
}

/// The resolution echo: what the approximate inputs snapped to, and what role each op resolved
/// to — the D2 doctrine's "recorded as exact data, echoed back".
pub struct ResolveReport<B: Backend = Bignum> {
    /// Per region: the requested azimuth degrees (if authored that way) and the exact snapped
    /// σ-band actually recorded.
    pub regions: Vec<RegionEcho<B>>,
    /// Per op: the derived role, and what its own cut certified to.
    pub ops: Vec<OpReport<B>>,
}

/// One region's echo.
pub struct RegionEcho<B: Backend = Bignum> {
    /// The requested azimuth degrees, when the region was authored in degrees.
    pub requested_deg: Option<(f64, f64)>,
    /// The exact recorded σ-band.
    pub band: Interval<B>,
}

/// One op's derived resolution.
pub struct OpReport<B: Backend = Bignum> {
    /// Whether the op subtracts (else intersects).
    pub subtract: bool,
    /// The derived role.
    pub role: OpRole,
    /// For a [`Hole`](OpRole::Hole): the certified `sup dist(emitted loop, {F = 0})` of **this
    /// op's own cut** — the largest over its loops, `None` for every other role.
    ///
    /// This is the number the milestone's soundness argument turns on and the one `eps()` hides.
    /// `eps()` is the max over every stage and the panel boundary usually dominates it, so a cut
    /// that certified loosely and one that certified perfectly report the same part-level ε. The
    /// per-piece bound folds in the σ-midpoint comparison against the fill rule's own boundary
    /// (`docs/cutter-extrude-design.md` §11.5), which is what makes a stepped-over event a loose
    /// bound rather than a wrong hole — so reading it is how one sees that the search is buying
    /// tightness and not soundness.
    pub cut_eps: Option<Rat<B>>,
    /// For a [`Hole`](OpRole::Hole): the widest µ̂ gap closed at a pinch or saddle of this op's
    /// loops. Included in [`cut_eps`](Self::cut_eps) — a component of the bound, not a residual
    /// beside it.
    pub tangent_gap: Option<Rat<B>>,
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
