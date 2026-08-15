//! The **sketch-extrude cutter**: a profile drawn in a rational [`Frame`], swept from a homogeneous
//! [`Apex`], pulled back to the surfaces it cuts.
//!
//! A cutter is *frame × profile × apex × span* (`docs/cutter-extrude-design.md`). This module owns
//! the first three; the σ-pullback and the certificate sit below it in [`crate::cut`], and the span
//! above it. The whole thing runs on one idea, [`Cast`]: a 3-D point's frame coordinates are a
//! rational quotient, so **any 2-D carrier equation becomes a 3-D surface by substituting it and
//! clearing the denominator**. A line clears to a plane; a circle clears to a quadric. That single
//! rule covers both carrier classes, both apex kinds, and gives every wall its own carrier's sign.
//!
//! ## One apex, not two extrusion modes
//!
//! A parallel extrusion *is* a projection from a point at infinity, so `Apex = [a : w]` covers both:
//! `w ≠ 0` is a finite cast point `a/w` (a draft angle), `w = 0` a direction (a straight drill).
//! `w == 0` is an **exact** `Rat` test — no float, no tolerance — and the cast takes the apex whole,
//! so the projection, the walls and the nappe selector are one formula each rather than pairs of
//! near-duplicates. Pushing a finite apex outward degrades the taper to parallel continuously, with
//! no API discontinuity.
//!
//! ## What a wall is
//!
//! The wall over a profile edge is the surface swept by the generatrices through it. Its class
//! follows the edge's **carrier** — never its endpoints, since a wall is unbounded either way and
//! trimming to the authored piece happens later, in the `(σ, µ̂)` domain:
//!
//! | profile carrier | apex at infinity (`w = 0`) | finite apex (`w ≠ 0`) |
//! |---|---|---|
//! | line | plane | plane (through the apex and the line) |
//! | circle | elliptic cylinder | elliptic cone |
//!
//! Both right-hand cells are [`CutSurface::Quadric`] — a circle cast from an off-axis apex is an
//! *oblique* cone, and an affine frame makes a profile circle an ellipse to begin with, so neither
//! is a metric cylinder or a right circular cone. Everything stays degree ≤ 2, so every wall pulls
//! back to a degree-≤2 rail over ℚ(σ) through [`cut_mu_form`](crate::cut::cut_mu_form).
//!
//! ## Two views, kept apart
//!
//! [`Cast::contains`] answers *is this point inside the cutter* by projecting and asking the
//! profile's own arrangement — exact, and blind to whether the profile is convex or has holes.
//! [`Cast::walls`] gives the boundary surfaces the certificate needs. The fill rule lives with the
//! region, not with the individual walls, so each wall simply **mirrors its own carrier's sign**.
//!
//! ```
//! use develop::extrude::{Apex, Cast, Frame};
//! use geom::content::Circle;
//! use lattice::{Bignum, Rat};
//!
//! type Q = Rat<Bignum>;
//! let q = |n: i128| Q::from_i128(n);
//! // A sketch in the z = 0 plane, cast from (0, 0, 4): a drafted round hole of radius 1.
//! let frame = Frame::new(
//!     [q(0), q(0), q(0)],
//!     [q(1), q(0), q(0)],
//!     [q(0), q(1), q(0)],
//! )?;
//! assert!(frame.metric().is_orthonormal()); // so the profile circle is a true circle
//! let cast = Cast::new(frame, Apex::point([q(0), q(0), q(4)]))?;
//!
//! // A point casts to its frame coordinates along its own generatrix.
//! assert_eq!(cast.coords(&[q(0), q(2), q(2)]), Some((q(0), q(4))));
//!
//! // The wall is negative inside the disc, and the taper is real: by height 2 the hole has
//! // narrowed to radius 1/2, so the same offset 3/4 is inside at the profile plane and outside
//! // two units up.
//! let wall = cast.circle_wall(&Circle { cx: q(0), cy: q(0), r2: q(1) })?;
//! assert!(wall.residual(&[q(0), Q::new(3, 4), q(0)]).unwrap().sign() < 0);
//! assert!(wall.residual(&[q(0), Q::new(3, 4), q(2)]).unwrap().sign() > 0);
//! assert!(wall.residual(&[q(0), Q::new(1, 4), q(2)]).unwrap().sign() < 0);
//! # Ok::<(), develop::extrude::ExtrudeFault>(())
//! ```

use crate::cut::{CutSurface, Nappe};
use arrange2d::boolean::Region;
use geom::content::{Circle, Edge, Line};
use lattice::{Backend, Bignum, Rat, Surd};

/// A rational 3-vector.
type V3<B> = [Rat<B>; 3];

/// Why a wall could not be built. Every one of these is a **refusal** — the authored data does not
/// describe a surface — never a tolerance that a finer subdivision would clear.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtrudeFault {
    /// The apex is `[0 : 0]`, which is not a point of projective space.
    DegenerateApex,
    /// The profile edge is degenerate: a segment with coincident endpoints, or an ellipse whose two
    /// conjugate semi-axes are parallel (so it spans no plane).
    DegenerateProfile,
    /// The apex lies **in** the profile's own plane (`N · (a − w·o) = 0`). A finite apex there sees
    /// the profile edge-on and casts no cone; a direction there extrudes parallel to the plane. This
    /// is the §4.1 apex-clearance condition at build time — refused, never repaired.
    ApexInPlane,
    /// The frame's two spanning vectors are parallel, so they span no plane and the sketch has no
    /// coordinates.
    DegenerateFrame,
    /// A profile circle with `r² ≤ 0` — not a circle. (Mirrors `arrange2d`'s own input fault: both
    /// store circles by squared radius, and both refuse the non-real case rather than take a root.)
    NonPositiveRadius,
}

/// The plane a sketch is drawn in: an origin and two independent spanning vectors, all rational.
/// A profile point `(a, b)` maps to `o + a·u + b·v`.
///
/// The frame is deliberately **affine**, not orthonormal. Rational orthonormal frames exist only for
/// special normals, so demanding `u ⊥ v` with `|u| = |v| = 1` would make a general picked frame
/// unrepresentable exactly — and exactness here is the whole point. The cost is stated rather than
/// hidden: a circle drawn in frame coordinates is a circle *in those coordinates*, and is a true
/// metric circle only when the frame happens to be orthonormal, which [`Frame::metric`] reports.
/// Where a true-metric frame is wanted, rational points on the unit sphere are dense, so a picked
/// normal can be snapped to a rational unit vector as closely as required.
#[derive(Debug)]
pub struct Frame<B: Backend = Bignum> {
    o: V3<B>,
    u: V3<B>,
    v: V3<B>,
}

impl<B: Backend> Clone for Frame<B> {
    fn clone(&self) -> Self {
        Frame {
            o: self.o.clone(),
            u: self.u.clone(),
            v: self.v.clone(),
        }
    }
}

impl<B: Backend> Frame<B> {
    /// The frame with origin `o` and spanning vectors `u, v`. Refuses parallel spanning vectors.
    pub fn new(o: V3<B>, u: V3<B>, v: V3<B>) -> Result<Self, ExtrudeFault> {
        if is_zero3(&cross3(&u, &v)) {
            return Err(ExtrudeFault::DegenerateFrame);
        }
        Ok(Frame { o, u, v })
    }

    /// The origin.
    pub fn origin(&self) -> &V3<B> {
        &self.o
    }

    /// The first spanning vector — the frame's `a` axis.
    pub fn u(&self) -> &V3<B> {
        &self.u
    }

    /// The second spanning vector — the frame's `b` axis.
    pub fn v(&self) -> &V3<B> {
        &self.v
    }

    /// The plane normal `u × v`. Nonzero, but **not** unit: `|u × v|` is generally irrational, and
    /// the certificates that need a true length take the `√` themselves.
    pub fn normal(&self) -> V3<B> {
        cross3(&self.u, &self.v)
    }

    /// The 3-D point at frame coordinates `(a, b)`.
    pub fn point(&self, a: &Rat<B>, b: &Rat<B>) -> V3<B> {
        add3(&self.o, &add3(&scale3(&self.u, a), &scale3(&self.v, b)))
    }

    /// How far this frame is from orthonormal — see [`FrameMetric`].
    pub fn metric(&self) -> FrameMetric<B> {
        FrameMetric {
            uu: dot3(&self.u, &self.u),
            uv: dot3(&self.u, &self.v),
            vv: dot3(&self.v, &self.v),
        }
    }

    /// The reciprocal basis `(u*, v*)` in the frame plane: the pair with `u*·u = v*·v = 1` and
    /// `u*·v = v*·u = 0`, which reads a plane vector's frame coordinates. Rational, via the 2×2 Gram
    /// inverse — whose determinant is `|u × v|² > 0` by the constructor's check.
    fn reciprocal(&self) -> (V3<B>, V3<B>) {
        let m = self.metric();
        let inv_det = m.uu.mul(&m.vv).sub(&m.uv.mul(&m.uv)).recip();
        (
            scale3(
                &sub3(&scale3(&self.u, &m.vv), &scale3(&self.v, &m.uv)),
                &inv_det,
            ),
            scale3(
                &sub3(&scale3(&self.v, &m.uu), &scale3(&self.u, &m.uv)),
                &inv_det,
            ),
        )
    }
}

/// A frame's Gram matrix — what it does to lengths and angles, reported rather than assumed.
///
/// A caller that needs a profile circle to be a *metric* circle should check
/// [`is_orthonormal`](Self::is_orthonormal) first; one that only needs exactness can ignore this
/// entirely, since every wall stays a quadric either way.
#[derive(Debug)]
pub struct FrameMetric<B: Backend = Bignum> {
    /// `u·u` — the squared length of the `a` axis.
    pub uu: Rat<B>,
    /// `u·v` — the **skew**: zero exactly when the axes are perpendicular.
    pub uv: Rat<B>,
    /// `v·v` — the squared length of the `b` axis.
    pub vv: Rat<B>,
}

impl<B: Backend> Clone for FrameMetric<B> {
    fn clone(&self) -> Self {
        FrameMetric {
            uu: self.uu.clone(),
            uv: self.uv.clone(),
            vv: self.vv.clone(),
        }
    }
}

impl<B: Backend> FrameMetric<B> {
    /// Whether the frame is exactly orthonormal, so frame coordinates are metric coordinates and a
    /// profile circle is a true circle.
    pub fn is_orthonormal(&self) -> bool {
        self.uv.is_zero()
            && self.uu.cmp(&Rat::from_i128(1)) == core::cmp::Ordering::Equal
            && self.vv.cmp(&Rat::from_i128(1)) == core::cmp::Ordering::Equal
    }

    /// The **anisotropy** `u·u − v·v`: zero exactly when the two axes are equally scaled. Together
    /// with `uv`, zero here means the frame is a similarity — angles and circles survive, lengths
    /// scale uniformly.
    pub fn anisotropy(&self) -> Rat<B> {
        self.uu.sub(&self.vv)
    }
}

/// The extrusion apex as **one homogeneous point** `[a : w]`.
///
/// `w ≠ 0` is the finite cast point `a/w`; `w = 0` is the direction `a`. See the module docs for
/// why these are one object rather than two variants.
#[derive(Debug)]
pub struct Apex<B: Backend = Bignum> {
    a: V3<B>,
    w: Rat<B>,
}

// Manual `Clone` throughout this module (no `B: Clone` bound — the `geom::content` idiom): the
// `Backend` implementors are marker types, and the `Rat` fields' own manual `Clone` does the work.
impl<B: Backend> Clone for Apex<B> {
    fn clone(&self) -> Self {
        Apex {
            a: self.a.clone(),
            w: self.w.clone(),
        }
    }
}

impl<B: Backend> Apex<B> {
    /// The finite cast point `p` — a drafted extrusion, whose generatrices all meet at `p`.
    pub fn point(p: V3<B>) -> Self {
        Self {
            a: p,
            w: Rat::from_i128(1),
        }
    }

    /// The direction `d` — a parallel extrusion, the apex at infinity. `None` for `d = 0`, which is
    /// not a direction.
    pub fn direction(d: V3<B>) -> Option<Self> {
        (!is_zero3(&d)).then_some(Self {
            a: d,
            w: Rat::from_i128(0),
        })
    }

    /// The raw homogeneous point `[a : w]`. `None` for `[0 : 0]`.
    pub fn homogeneous(a: V3<B>, w: Rat<B>) -> Option<Self> {
        (!is_zero3(&a) || !w.is_zero()).then_some(Self { a, w })
    }

    /// The homogeneous numerator `a`.
    pub fn a(&self) -> &V3<B> {
        &self.a
    }

    /// The homogeneous weight `w`.
    pub fn w(&self) -> &Rat<B> {
        &self.w
    }

    /// Whether this apex is at infinity (`w == 0`) — an **exact** rational test.
    pub fn is_direction(&self) -> bool {
        self.w.is_zero()
    }

    /// The finite cast point `a/w`, or `None` for a direction.
    pub fn finite(&self) -> Option<V3<B>> {
        (!self.is_direction()).then(|| scale3(&self.a, &self.w.recip()))
    }

    /// `a − w·p`, the apex taken relative to `p`. This is the one expression both builders need: at
    /// `w = 0` it is the direction, at `w = 1` it is `apex − p`, and in general it is `w·(a/w − p)`
    /// — the same vector up to a positive-or-negative scale, which every use below squares or
    /// crosses away.
    fn relative_to(&self, p: &V3<B>) -> V3<B> {
        sub3(&self.a, &scale3(p, &self.w))
    }
}

/// The **frame-and-apex projection** — everything needed to pull a 2-D carrier back to the surface
/// it sweeps in 3-D, and to answer where a 3-D point lands in the sketch.
///
/// This is the engine the whole profile path runs on. A point `X` casts to the frame plane along the
/// generatrix through the apex, and its frame coordinates come out as a single rational quotient
///
/// ```text
/// (a, b) = ( L₁·(X − o) / T(X) ,  L₂·(X − o) / T(X) )
/// ```
///
/// with `L₁, L₂` rational 3-vectors and `T` a rational affine form — the projected point's
/// homogeneous weight. Every wall is then just a **2-D carrier equation with the quotient
/// substituted and the denominator cleared**: a line clears to a plane, a circle clears to a
/// quadric. One derivation covers both carrier classes and both apex kinds, and each wall inherits
/// **its own carrier's sign** — `< 0` where the 2-D carrier is negative, so a circle is negative
/// inside its disc, matching [`CutSurface`]'s convention with nothing to fix up.
///
/// `T` vanishes exactly on the plane through the apex parallel to the frame, which is where a
/// generatrix runs parallel to the frame plane and no projection exists. That plane is also the
/// boundary of the [`Nappe`] (§4.1), so the same quantity carries both meanings.
pub struct Cast<B: Backend = Bignum> {
    frame: Frame<B>,
    apex: Apex<B>,
    /// `N = u × v`, the frame normal (not unit).
    n: V3<B>,
    /// `K = N·(a − w·o)` — the apex against the frame plane. Nonzero by construction.
    k: Rat<B>,
    /// The frame-coordinate numerators, `L₁` and `L₂`.
    l: [V3<B>; 2],
    /// `T(X) = τ·X + t`, the projected weight.
    tau: V3<B>,
    t: Rat<B>,
}

impl<B: Backend> Cast<B> {
    /// Build the projection. Refuses an apex lying **in** the frame plane, where a finite apex sees
    /// the sketch edge-on and a direction extrudes parallel to it (§4.1, at build time).
    pub fn new(frame: Frame<B>, apex: Apex<B>) -> Result<Self, ExtrudeFault> {
        check_apex(&apex)?;
        let n = frame.normal();
        let ap = apex.relative_to(&frame.o);
        let k = dot3(&n, &ap);
        if k.is_zero() {
            return Err(ExtrudeFault::ApexInPlane);
        }
        // `L_i = K·(reciprocal axis) − (a'·reciprocal axis)·N`.
        let (ru, rv) = frame.reciprocal();
        let el = |r: &V3<B>| sub3(&scale3(r, &k), &scale3(&n, &dot3(&ap, r)));
        let l = [el(&ru), el(&rv)];
        let tau = scale3(&n, &apex.w.neg());
        let t = k.add(&apex.w.mul(&dot3(&n, &frame.o)));
        Ok(Cast {
            frame,
            apex,
            n,
            k,
            l,
            tau,
            t,
        })
    }

    /// The frame the sketch is drawn in.
    pub fn frame(&self) -> &Frame<B> {
        &self.frame
    }

    /// The apex the sketch is cast from.
    pub fn apex(&self) -> &Apex<B> {
        &self.apex
    }

    /// The frame normal `u × v` (not unit).
    pub fn normal(&self) -> &V3<B> {
        &self.n
    }

    /// The [`Nappe`] every wall of this cast carries: the side of the apex plane the sketch is on.
    /// Vacuously true for a direction apex, which has no second nappe.
    pub fn nappe(&self) -> Nappe<B> {
        Nappe {
            n: scale3(&self.tau, &self.k),
            d: self.k.mul(&self.t).neg(),
        }
    }

    /// The projected weight `T(X)` — positive-or-negative according to which nappe `X` is on, and
    /// exactly zero where no projection exists.
    pub fn weight(&self, x: &V3<B>) -> Rat<B> {
        dot3(&self.tau, x).add(&self.t)
    }

    /// Where `X` lands in frame coordinates, casting along its generatrix. `None` exactly when the
    /// generatrix is parallel to the frame plane (`T(X) = 0`), which includes the apex itself.
    pub fn coords(&self, x: &V3<B>) -> Option<(Rat<B>, Rat<B>)> {
        let w = self.weight(x);
        if w.is_zero() {
            return None;
        }
        let d = sub3(x, &self.frame.o);
        Some((dot3(&self.l[0], &d).div(&w), dot3(&self.l[1], &d).div(&w)))
    }

    /// The wall swept by the generatrices through a frame-plane **line** `α·a + β·b + γ = 0`: the
    /// plane through that line and the apex.
    ///
    /// Substituting the frame-coordinate quotient and clearing `T` leaves
    /// `α·L₁·(X−o) + β·L₂·(X−o) + γ·T(X)`, which is affine in `X`. The returned plane is scaled so
    /// its residual is **negative exactly where the 2-D line's is**, on the authored nappe.
    pub fn line_wall(&self, line: &Line<B>) -> CutSurface<B> {
        let (a, b, c) = (&line.a, &line.b, &line.c);
        let lin = add3(&scale3(&self.l[0], a), &scale3(&self.l[1], b));
        // `sign(T) = sign(K)` on the authored nappe, so this puts the wall in step with the carrier.
        let s = Rat::from_i128(self.k.sign() as i128);
        let n = scale3(&add3(&lin, &scale3(&self.tau, c)), &s);
        let d = dot3(&lin, &self.frame.o).sub(&c.mul(&self.t)).mul(&s);
        CutSurface::Plane { n, d }
    }

    /// The wall swept by the generatrices through a frame-plane **circle** `(a−cx)² + (b−cy)² = r²`:
    /// an elliptic cone for a finite apex, an elliptic cylinder for a direction.
    ///
    /// Only `r²` is needed — never the generally-irrational `r` — because clearing `T²` from the
    /// circle equation leaves `(L₁·(X−o) − cx·T)² + (L₂·(X−o) − cy·T)² − r²·T²`, in which `r²`
    /// appears linearly. That is what lets a profile arc come straight from `arrange2d`, whose
    /// circles are stored by squared radius for the same reason.
    ///
    /// The residual is `T²` times the 2-D circle's, so it is **negative inside the disc** for free.
    /// Refuses `r² ≤ 0`, which is not a circle.
    pub fn circle_wall(&self, circle: &Circle<B>) -> Result<CutSurface<B>, ExtrudeFault> {
        if circle.r2.sign() <= 0 {
            return Err(ExtrudeFault::NonPositiveRadius);
        }
        // The cleared circle is the weighted sum of three squared affine forms in `X`:
        // `F = A₀² + A₁² − r²·T²`, with `A_i = L_i·(X−o) − c_i·T`.
        let centred = |i: usize, c: &Rat<B>| {
            (
                sub3(&self.l[i], &scale3(&self.tau, c)),
                dot3(&self.l[i], &self.frame.o).neg().sub(&c.mul(&self.t)),
            )
        };
        let terms = [
            (centred(0, &circle.cx), Rat::from_i128(1)),
            (centred(1, &circle.cy), Rat::from_i128(1)),
            ((self.tau.clone(), self.t.clone()), circle.r2.neg()),
        ];
        let mut m: [[Rat<B>; 3]; 3] = core::array::from_fn(|_| zero3());
        let mut b: V3<B> = zero3();
        let mut c = Rat::from_i128(0);
        for ((v, k), w) in &terms {
            // `w·(v·X + k)² = Xᵀ(w·vvᵀ)X + 2wk·(v·X) + wk²`.
            m = add3x3(&m, &scale3x3(&outer3(v, v), w));
            b = add3(&b, &scale3(v, &k.mul(w).mul(&Rat::from_i128(2))));
            c = c.add(&k.mul(k).mul(w));
        }
        Ok(CutSurface::Quadric(Box::new(crate::cut::Quadric {
            m,
            b,
            c,
            nappe: self.nappe(),
        })))
    }

    /// The wall of one profile edge, read off its **carrier** — a segment's line, an arc's circle.
    /// The edge's endpoints play no part: a wall is an unbounded surface either way, and trimming to
    /// the authored piece happens in the `(σ, µ̂)` domain, where it is a boolean rather than a
    /// surface property.
    pub fn edge_wall(&self, edge: &Edge<B>) -> Result<CutSurface<B>, ExtrudeFault> {
        match edge {
            Edge::Seg(s) => {
                if s.line.a.is_zero() && s.line.b.is_zero() {
                    return Err(ExtrudeFault::DegenerateProfile);
                }
                Ok(self.line_wall(&s.line))
            }
            Edge::Arc(a) => self.circle_wall(&a.circle),
        }
    }

    /// Every edge's wall, in order. Carriers are **not** deduplicated: two pieces of one line give
    /// two identical walls, which is what the caller wants when it is walking edges.
    pub fn walls(&self, edges: &[Edge<B>]) -> Result<Vec<CutSurface<B>>, ExtrudeFault> {
        edges.iter().map(|e| self.edge_wall(e)).collect()
    }

    /// Is the 3-D point `X` inside the sketch — does its generatrix pass through the profile?
    ///
    /// This is the **predicate view** of the cutter: cast `X` to frame coordinates, then ask the
    /// profile's own arrangement. Both steps are exact, so the answer is exact; and because it reads
    /// the region's even-odd fill, non-convex profiles and holes need no decomposition.
    ///
    /// `None` when the question is not decidable at this point, which is not a defect but a
    /// precondition the caller must respect — it happens when the generatrix through `X` is parallel
    /// to the frame plane (no projection), or when the projected row `b` is one that exact
    /// ray-casting excludes (a circle centre's row, or a vertex row — see
    /// [`arrange2d::locate`](arrange2d::locate)). Re-query at a nudged sample.
    pub fn contains(&self, x: &V3<B>, edges: &[Edge<B>]) -> Option<bool> {
        let (a, b) = self.coords(x)?;
        generic_row(&b, edges).then(|| arrange2d::locate::winding_parity(&a, &b, edges))
    }
}

/// Does the horizontal row `y` satisfy the exact ray-casting genericity precondition — clear of
/// every circle centre and every edge endpoint?
fn generic_row<B: Backend>(y: &Rat<B>, edges: &[Edge<B>]) -> bool {
    let ys = Surd::from_rat(y.clone());
    edges.iter().all(|e| {
        let (start, end) = match e {
            Edge::Seg(s) => (&s.start, &s.end),
            Edge::Arc(a) => {
                if a.circle.cy.cmp(y) == core::cmp::Ordering::Equal {
                    return false;
                }
                (&a.start, &a.end)
            }
        };
        start.y.cmp(&ys) != core::cmp::Ordering::Equal
            && end.y.cmp(&ys) != core::cmp::Ordering::Equal
    })
}

/// Every boundary edge of a profile region — all faces, outer loops and holes alike.
///
/// The even-odd fill that [`Cast::contains`] reads does not care which loop an edge came from, so
/// flattening is lossless: nesting is recovered by the parity count, which is exactly why a profile
/// with holes needs no decomposition.
pub fn region_edges<B: Backend>(region: &Region<B>) -> Vec<Edge<B>> {
    region
        .faces
        .iter()
        .flat_map(|f| f.outer.iter().chain(f.holes.iter().flatten()))
        .cloned()
        .collect()
}

/// The wall swept by the generatrices through the ellipse `Q + cos t·e1 + sin t·e2`: an **elliptic
/// cone** for a finite apex, an **elliptic cylinder** for a direction.
///
/// Sugar over [`Cast::circle_wall`] — the ellipse is the unit circle of the frame `(Q; e1, e2)`, so
/// this is the same derivation rather than a second one. `e1` and `e2` are conjugate semi-axes: any
/// two independent in-plane vectors, not necessarily orthogonal and not necessarily of equal length.
///
/// ```
/// use develop::extrude::{Apex, ellipse_wall};
/// use lattice::{Bignum, Rat};
///
/// type Q = Rat<Bignum>;
/// let q = |n: i128| Q::from_i128(n);
/// // The unit circle in the z = 0 plane, cast from the apex (0,0,4): a right circular cone.
/// let wall = ellipse_wall(
///     &[q(0), q(0), q(0)],
///     &[q(1), q(0), q(0)],
///     &[q(0), q(1), q(0)],
///     &Apex::point([q(0), q(0), q(4)]),
/// )
/// .expect("a real cone");
/// let f = |p: [Q; 3]| wall.residual(&p).expect("a well-formed wall has a residual");
/// // The generating circle lies on it, exactly.
/// assert!(f([q(1), q(0), q(0)]).is_zero());
/// assert!(f([Q::new(3, 5), Q::new(4, 5), q(0)]).is_zero());
/// // Halfway to the apex the cone has halved: (1/2, 0, 2) is on it, (1, 0, 2) is outside.
/// assert!(f([Q::new(1, 2), q(0), q(2)]).is_zero());
/// assert!(f([q(1), q(0), q(2)]).sign() > 0);
/// ```
pub fn ellipse_wall<B: Backend>(
    center: &V3<B>,
    e1: &V3<B>,
    e2: &V3<B>,
    apex: &Apex<B>,
) -> Result<CutSurface<B>, ExtrudeFault> {
    let frame = Frame::new(center.clone(), e1.clone(), e2.clone())?;
    Cast::new(frame, apex.clone())?.circle_wall(&Circle {
        cx: Rat::from_i128(0),
        cy: Rat::from_i128(0),
        r2: Rat::from_i128(1),
    })
}

/// `[0 : 0]` is not a projective point.
fn check_apex<B: Backend>(apex: &Apex<B>) -> Result<(), ExtrudeFault> {
    if is_zero3(&apex.a) && apex.w.is_zero() {
        return Err(ExtrudeFault::DegenerateApex);
    }
    Ok(())
}

// --- exact rational vector helpers ------------------------------------------------------------

/// The zero vector. (`Rat` is deliberately not `Copy`, so `[Rat::zero(); 3]` is unavailable.)
fn zero3<B: Backend>() -> V3<B> {
    core::array::from_fn(|_| Rat::from_i128(0))
}

/// Whether every component is exactly zero.
fn is_zero3<B: Backend>(a: &V3<B>) -> bool {
    a.iter().all(|x| x.is_zero())
}

/// `a · b`.
fn dot3<B: Backend>(a: &V3<B>, b: &V3<B>) -> Rat<B> {
    a[0].mul(&b[0]).add(&a[1].mul(&b[1])).add(&a[2].mul(&b[2]))
}

/// `a × b`.
fn cross3<B: Backend>(a: &V3<B>, b: &V3<B>) -> V3<B> {
    [
        a[1].mul(&b[2]).sub(&a[2].mul(&b[1])),
        a[2].mul(&b[0]).sub(&a[0].mul(&b[2])),
        a[0].mul(&b[1]).sub(&a[1].mul(&b[0])),
    ]
}

/// `a + b`.
fn add3<B: Backend>(a: &V3<B>, b: &V3<B>) -> V3<B> {
    [a[0].add(&b[0]), a[1].add(&b[1]), a[2].add(&b[2])]
}

/// `a − b`.
fn sub3<B: Backend>(a: &V3<B>, b: &V3<B>) -> V3<B> {
    [a[0].sub(&b[0]), a[1].sub(&b[1]), a[2].sub(&b[2])]
}

/// `k·a`.
fn scale3<B: Backend>(a: &V3<B>, k: &Rat<B>) -> V3<B> {
    [a[0].mul(k), a[1].mul(k), a[2].mul(k)]
}

/// The outer product `a bᵀ`.
fn outer3<B: Backend>(a: &V3<B>, b: &V3<B>) -> [[Rat<B>; 3]; 3] {
    core::array::from_fn(|i| core::array::from_fn(|j| a[i].mul(&b[j])))
}

/// `A + B`.
fn add3x3<B: Backend>(a: &[[Rat<B>; 3]; 3], b: &[[Rat<B>; 3]; 3]) -> [[Rat<B>; 3]; 3] {
    core::array::from_fn(|i| core::array::from_fn(|j| a[i][j].add(&b[i][j])))
}

/// `k·A`.
fn scale3x3<B: Backend>(a: &[[Rat<B>; 3]; 3], k: &Rat<B>) -> [[Rat<B>; 3]; 3] {
    core::array::from_fn(|i| core::array::from_fn(|j| a[i][j].mul(k)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    fn q(n: i128) -> Q {
        Q::from_i128(n)
    }

    /// The origin.
    fn o3() -> [Q; 3] {
        [q(0), q(0), q(0)]
    }

    /// The fault a refused wall carries. (`CutSurface` is deliberately not `Debug`/`PartialEq` — it
    /// is geometry, not a value to compare — so the `Ok` side is dropped rather than unwrapped.)
    fn fault(r: Result<CutSurface<Bignum>, ExtrudeFault>) -> Option<ExtrudeFault> {
        r.err()
    }

    /// A rational as an `f64`, for assertion messages only — never for a decision.
    fn to_f64(r: &Q) -> f64 {
        let (n, d) = r.numer_denom_decimal();
        n.parse::<f64>().unwrap_or(f64::NAN) / d.parse::<f64>().unwrap_or(f64::NAN)
    }

    /// The residual of a wall built here — always defined, since these are all real surfaces.
    fn res(wall: &CutSurface<Bignum>, p: &[Q; 3]) -> Q {
        wall.residual(p).expect("a well-formed wall has a residual")
    }

    /// Rational points on the unit circle — exact `(cos t, sin t)` samples, so "the profile lies on
    /// its own wall" is testable with `== 0`, not with a tolerance.
    const CIRCLE: [(i128, i128, i128); 6] = [
        (1, 0, 1),
        (0, 1, 1),
        (3, 4, 5),
        (-4, 3, 5),
        (-5, -12, 13),
        (8, -15, 17),
    ];

    fn apex_pt(x: i128, y: i128, z: i128) -> Apex<Bignum> {
        Apex::point([q(x), q(y), q(z)])
    }

    fn apex_dir(x: i128, y: i128, z: i128) -> Apex<Bignum> {
        Apex::direction([q(x), q(y), q(z)]).expect("nonzero direction")
    }

    #[test]
    fn apex_is_one_projective_point() {
        let p = apex_pt(1, 2, 3);
        assert!(!p.is_direction());
        assert_eq!(p.finite().unwrap()[2], q(3));
        // `w == 0` is exact: a cast point pushed a billion units out is still finite.
        let far = apex_pt(0, 0, 1_000_000_000);
        assert!(!far.is_direction());

        let d = apex_dir(0, 0, 1);
        assert!(d.is_direction());
        assert!(d.finite().is_none());

        assert!(Apex::<Bignum>::direction(o3()).is_none());
        assert!(Apex::<Bignum>::homogeneous(o3(), q(0)).is_none());
        // A scaled homogeneous point is the same cast point.
        let h = Apex::<Bignum>::homogeneous([q(2), q(4), q(6)], q(2)).unwrap();
        assert_eq!(h.finite().unwrap(), apex_pt(1, 2, 3).finite().unwrap());
    }

    /// Every wall contains the edge that generates it — exactly, for both apex kinds.
    #[test]
    fn walls_contain_their_profile_edge() {
        let centre = [q(1), q(-2), q(3)];
        let (e1, e2) = ([q(2), q(0), q(0)], [q(0), q(3), q(0)]); // a genuine ellipse, not a circle
        for apex in [apex_pt(5, 7, 11), apex_dir(1, 2, 5), apex_pt(0, 0, 40)] {
            let wall = ellipse_wall(&centre, &e1, &e2, &apex).expect("a real wall");
            for (cn, sn, den) in CIRCLE {
                let (c, s) = (Q::new(cn, den), Q::new(sn, den));
                let p = add3(&centre, &add3(&scale3(&e1, &c), &scale3(&e2, &s)));
                assert!(res(&wall, &p).is_zero(), "ellipse point off its own wall");
            }
            // The apex itself is the cone's vertex (and lies at infinity for a direction).
            if let Some(a) = apex.finite() {
                assert!(res(&wall, &a).is_zero(), "apex off its own cone");
            }
        }
    }

    /// A line wall contains the line it was built from and the apex, exactly — the property the
    /// certificate rests on, stated where the wall is actually built.
    #[test]
    fn a_line_wall_contains_its_line_and_its_apex() {
        let cast = skew_cast();
        // `2a − b + 3 = 0` in frame coordinates: two points on it, exactly.
        let line = Line {
            a: q(2),
            b: q(-1),
            c: q(3),
        };
        for a in [q(0), q(1), Q::new(-7, 3)] {
            let b = line.a.mul(&a).add(&line.c).div(&line.b.neg());
            let p = cast.frame().point(&a, &b);
            let wall = cast.line_wall(&line);
            assert!(
                wall.residual(&p).unwrap().is_zero(),
                "a point of the line is on its own wall"
            );
            let ap = cast.apex().finite().unwrap();
            assert!(wall.residual(&ap).unwrap().is_zero(), "so is the apex");
            // Negating the carrier negates the wall — the sign is the carrier's, nothing else.
            let back = cast.line_wall(&Line {
                a: line.a.neg(),
                b: line.b.neg(),
                c: line.c.neg(),
            });
            let probe = cast.frame().point(&q(5), &q(-4));
            assert!(
                wall.residual(&probe)
                    .unwrap()
                    .add(&back.residual(&probe).unwrap())
                    .is_zero()
            );
        }
    }

    /// The interior of the cone is `F < 0` — [`CutSurface`]'s convention — and the mirror nappe is
    /// excluded by the selector rather than by the residual.
    #[test]
    fn cone_interior_is_negative_and_the_mirror_nappe_is_selected_out() {
        let apex = apex_pt(0, 0, 4);
        let wall = ellipse_wall(&o3(), &[q(1), q(0), q(0)], &[q(0), q(1), q(0)], &apex)
            .expect("a real cone");
        // On the axis between the plane and the apex: strictly inside.
        assert!(res(&wall, &[q(0), q(0), q(1)]).sign() < 0);
        // Far off-axis in the same plane: strictly outside.
        assert!(res(&wall, &[q(3), q(0), q(1)]).sign() > 0);
        // The mirror nappe: the residual vanishes (it is the same double cone) but the nappe
        // selector rejects it. z = 8 is one unit beyond the apex, where the mirror radius is 1.
        let mirror = [q(1), q(0), q(8)];
        assert!(res(&wall, &mirror).is_zero());
        assert!(!wall.on_nappe(&mirror));
        assert!(wall.on_nappe(&[q(1), q(0), q(0)]));
    }

    /// A cylinder has no wrong nappe, and the vacuous selector says so for every point.
    #[test]
    fn a_cylinder_has_no_nappe_to_get_wrong() {
        let wall = ellipse_wall(
            &o3(),
            &[q(1), q(0), q(0)],
            &[q(0), q(1), q(0)],
            &apex_dir(0, 0, 1),
        )
        .expect("a real cylinder");
        for z in [-1000, 0, 1000] {
            assert!(wall.on_nappe(&[q(1), q(0), q(z)]));
            assert!(res(&wall, &[q(1), q(0), q(z)]).is_zero());
        }
    }

    #[test]
    fn degenerate_authoring_is_refused() {
        let e1 = [q(1), q(0), q(0)];
        let e2 = [q(0), q(1), q(0)];
        let o = o3();
        // The apex sits in the profile plane: no cone.
        assert_eq!(
            fault(ellipse_wall(&o, &e1, &e2, &apex_pt(9, 0, 0)),),
            Some(ExtrudeFault::ApexInPlane)
        );
        // The extrusion runs parallel to the profile plane.
        assert_eq!(
            fault(ellipse_wall(&o, &e1, &e2, &apex_dir(1, 1, 0)),),
            Some(ExtrudeFault::ApexInPlane)
        );
        // Parallel semi-axes span no plane — the ellipse's own frame is degenerate.
        assert_eq!(
            fault(ellipse_wall(
                &o,
                &e1,
                &[q(2), q(0), q(0)],
                &apex_dir(0, 0, 1)
            ),),
            Some(ExtrudeFault::DegenerateFrame)
        );
        // A profile edge whose carrier has no direction, and a circle that is not one.
        let cast = skew_cast();
        let mut flat = poly_edges(&[(0, 0), (1, 0), (0, 1)], 0);
        if let Edge::Seg(seg) = &mut flat[0] {
            seg.line.a = q(0);
            seg.line.b = q(0);
        }
        assert_eq!(
            cast.edge_wall(&flat[0]).err(),
            Some(ExtrudeFault::DegenerateProfile)
        );
        assert_eq!(
            cast.circle_wall(&Circle {
                cx: q(0),
                cy: q(0),
                r2: q(0)
            })
            .err(),
            Some(ExtrudeFault::NonPositiveRadius)
        );
        assert_eq!(
            Frame::new(o3(), e1.clone(), scale3(&e1, &q(2))).err(),
            Some(ExtrudeFault::DegenerateFrame)
        );
    }

    // --- frame, profile and the predicate view (AUTH.1b) ---------------------------------------

    /// A closed CCW loop of segment edges through the given frame-coordinate vertices. The directed
    /// line's leftward normal is `(−Δb, Δa)`, so a CCW loop's interior sits where the residual is
    /// **positive** — which is what [`Cast::line_wall`] then mirrors.
    fn poly_edges(pts: &[(i128, i128)], src: u32) -> Vec<Edge<Bignum>> {
        let n = pts.len();
        (0..n)
            .map(|i| {
                let ((sx, sy), (ex, ey)) = (pts[i], pts[(i + 1) % n]);
                let (a, b) = (q(-(ey - sy)), q(ex - sx));
                let c = a.mul(&q(sx)).add(&b.mul(&q(sy))).neg();
                Edge::Seg(Box::new(geom::content::SegPiece {
                    line: Line { a, b, c },
                    start: geom::content::Point2::from_rat(q(sx), q(sy)),
                    end: geom::content::Point2::from_rat(q(ex), q(ey)),
                    orient: geom::content::Orient::Ccw,
                    source: geom::content::CurveId(src),
                }))
            })
            .collect()
    }

    /// A whole circle as its two x-monotone halves — `arrange2d`'s canonical decomposition. `r` is
    /// passed rationally only so the test can name the two extreme points; the wall builder never
    /// sees it, taking `r²` alone.
    fn circle_edges(cx: Q, cy: Q, r: Q, src: u32) -> Vec<Edge<Bignum>> {
        let (lo, hi) = (cx.sub(&r), cx.add(&r));
        let circle = Circle {
            cx: cx.clone(),
            cy: cy.clone(),
            r2: r.mul(&r),
        };
        [geom::content::Half::Upper, geom::content::Half::Lower]
            .into_iter()
            .map(|half| {
                Edge::Arc(Box::new(geom::content::ArcPiece {
                    circle: circle.clone(),
                    half,
                    x_lo: Surd::from_rat(lo.clone()),
                    x_hi: Surd::from_rat(hi.clone()),
                    start: geom::content::Point2::from_rat(lo.clone(), cy.clone()),
                    end: geom::content::Point2::from_rat(hi.clone(), cy.clone()),
                    winding: geom::content::Winding {
                        orient: geom::content::Orient::Ccw,
                        source_span: None,
                    },
                    source: geom::content::CurveId(src),
                }))
            })
            .collect()
    }

    /// A frame that is emphatically not orthonormal, and a cast point off its plane.
    fn skew_frame() -> Frame<Bignum> {
        Frame::new([q(1), q(-2), q(3)], [q(2), q(0), q(1)], [q(0), q(3), q(1)])
            .expect("independent axes")
    }

    fn skew_cast() -> Cast<Bignum> {
        let frame = skew_frame();
        let apex = Apex::point(add3(frame.origin(), &frame.normal()));
        Cast::new(frame, apex).expect("the apex is off the plane")
    }

    /// The frame reports its metric rather than assuming one — and the report is not cosmetic: under
    /// a non-orthonormal frame a profile "circle" really is an ellipse in 3-D.
    #[test]
    fn an_affine_frame_reports_what_it_does_to_a_circle() {
        let metric = |u: [Q; 3], v: [Q; 3]| Frame::new(o3(), u, v).expect("independent").metric();

        let ortho = metric([q(1), q(0), q(0)], [q(0), q(1), q(0)]);
        assert!(ortho.is_orthonormal());
        assert!(ortho.uv.is_zero() && ortho.anisotropy().is_zero());

        // Perpendicular but unequally scaled: angle-preserving, not length-preserving.
        let scaled = metric([q(2), q(0), q(0)], [q(0), q(1), q(0)]);
        assert!(!scaled.is_orthonormal());
        assert!(scaled.uv.is_zero() && !scaled.anisotropy().is_zero());

        // Sheared: not even angle-preserving.
        let sheared = metric([q(1), q(0), q(0)], [q(1), q(1), q(0)]);
        assert!(!sheared.is_orthonormal() && !sheared.uv.is_zero());

        // The substance: the frame circle's two axis points sit at different true distances from the
        // centre, so it is an ellipse in 3-D. The anisotropy, made visible.
        let f = Frame::new(o3(), [q(2), q(0), q(0)], [q(0), q(1), q(0)]).unwrap();
        let (one, zero) = (q(1), q(0));
        let da = f.point(&one, &zero);
        let db = f.point(&zero, &one);
        assert!(dot3(&da, &da).cmp(&dot3(&db, &db)) != core::cmp::Ordering::Equal);
    }

    /// The cast is a projection along generatrices: a plane point maps to its own frame
    /// coordinates, every point of a generatrix maps to that same pair, and the map is undefined
    /// exactly on the apex's own plane.
    #[test]
    fn the_cast_projects_along_its_generatrix() {
        let parallel = {
            let frame = skew_frame();
            let n = frame.normal();
            Cast::new(frame, Apex::direction(n).unwrap()).unwrap()
        };
        for cast in [skew_cast(), parallel] {
            let (a, b) = (Q::new(3, 7), Q::new(-5, 4));
            let p = cast.frame().point(&a, &b);
            let got = cast.coords(&p).expect("a plane point projects to itself");
            assert!(got.0.sub(&a).is_zero() && got.1.sub(&b).is_zero());

            // Anywhere along the generatrix through `p`, the same pair comes back.
            for lam in [Q::new(1, 3), q(2), q(-1)] {
                let x = match cast.apex().finite() {
                    Some(ap) => add3(&ap, &scale3(&sub3(&p, &ap), &lam)),
                    None => add3(&p, &scale3(cast.apex().a(), &lam)),
                };
                let (ga, gb) = cast.coords(&x).expect("a generatrix point projects");
                assert!(ga.sub(&a).is_zero() && gb.sub(&b).is_zero());
            }
        }

        // On the apex's own plane there is no projection — the generatrix runs parallel to the
        // frame. For a finite apex that plane contains the apex itself.
        let cast = skew_cast();
        let apex = cast.apex().finite().unwrap();
        assert!(cast.coords(&apex).is_none());
        assert!(cast.coords(&add3(&apex, cast.frame().u())).is_none());
    }

    /// Every wall carries **its own carrier's sign**: the 3-D residual and the 2-D carrier residual
    /// at the projected point agree, everywhere on the authored nappe. That is the contract the
    /// assembly relies on, and it is what lets a profile's fill rule transfer unchanged.
    #[test]
    fn a_wall_mirrors_the_sign_of_its_own_carrier() {
        let cast = skew_cast();
        let line = Line {
            a: q(2),
            b: q(-1),
            c: q(3),
        };
        let circle = Circle {
            cx: q(1),
            cy: q(-2),
            r2: q(4),
        };
        let walls = [
            (cast.line_wall(&line), None),
            (cast.circle_wall(&circle).expect("r² > 0"), Some(&circle)),
        ];
        let ap = cast.apex().finite().unwrap();
        for (wall, circ) in &walls {
            for ai in -3..=3 {
                for bi in -3..=3 {
                    for lam in [Q::new(1, 2), q(1), q(3)] {
                        let (a, b) = (q(ai), q(bi));
                        let p = cast.frame().point(&a, &b);
                        let x = add3(&ap, &scale3(&sub3(&p, &ap), &lam));
                        assert!(wall.on_nappe(&x), "λ > 0 stays on the authored nappe");
                        let want = match circ {
                            Some(c) => a
                                .sub(&c.cx)
                                .mul(&a.sub(&c.cx))
                                .add(&b.sub(&c.cy).mul(&b.sub(&c.cy)))
                                .sub(&c.r2),
                            None => line.a.mul(&a).add(&line.b.mul(&b)).add(&line.c),
                        };
                        let got = wall.residual(&x).expect("a well-formed wall");
                        assert_eq!(
                            got.sign(),
                            want.sign(),
                            "wall sign disagrees with its carrier at ({ai}, {bi}), λ={}",
                            to_f64(&lam)
                        );
                    }
                }
            }
        }
    }

    /// **The predicate view.** A non-convex profile with a hole is decided by the region's own
    /// even-odd fill, so neither the concavity nor the hole needs any decomposition — and the whole
    /// thing is read through a *drafted* cast, so the projection is doing real work.
    #[test]
    fn a_non_convex_profile_with_a_hole_needs_no_decomposition() {
        let cast = skew_cast();
        // An L, plus a circular hole in its foot.
        let mut edges = poly_edges(&[(0, 0), (4, 0), (4, 2), (2, 2), (2, 4), (0, 4)], 0);
        edges.extend(circle_edges(q(1), q(1), Q::new(1, 2), 1));

        let ap = cast.apex().finite().unwrap();
        let at = |a: &Q, b: &Q, lam: &Q| {
            let p = cast.frame().point(a, b);
            add3(&ap, &scale3(&sub3(&p, &ap), lam))
        };
        for (a, b, want) in [
            (q(1), q(3), true),          // the upright arm
            (q(3), Q::new(1, 2), true),  // the foot, clear of the hole
            (q(1), Q::new(5, 2), true),  // the corner region
            (q(3), q(3), false),         // the notch the L cuts away
            (q(5), Q::new(1, 2), false), // outside altogether
            (q(1), Q::new(5, 4), false), // inside the hole ⇒ outside the cutter
        ] {
            for lam in [Q::new(1, 4), q(1), q(4)] {
                assert_eq!(
                    cast.contains(&at(&a, &b, &lam), &edges),
                    Some(want),
                    "({}, {}) at λ={}",
                    to_f64(&a),
                    to_f64(&b),
                    to_f64(&lam)
                );
            }
        }
        // Fail-closed rather than wrong: the hole's centre row is one exact ray-casting excludes,
        // and a point with no projection at all has no answer either.
        assert_eq!(cast.contains(&at(&q(3), &q(1), &q(1)), &edges), None);
        assert_eq!(cast.contains(&ap, &edges), None);
    }

    /// A region flattens losslessly for the predicate: parity recovers the nesting, so a hole is
    /// still a hole after its loop has been thrown in with the outer boundary.
    #[test]
    fn a_region_flattens_to_every_boundary_edge() {
        let outer = poly_edges(&[(0, 0), (4, 0), (4, 4), (0, 4)], 0);
        let hole = circle_edges(q(2), q(2), q(1), 1);
        let region = Region {
            faces: vec![arrange2d::boolean::Face {
                outer: outer.clone(),
                holes: vec![hole.clone()],
            }],
        };
        let edges = region_edges(&region);
        assert_eq!(edges.len(), outer.len() + hole.len());

        let cast = skew_cast();
        let ap = cast.apex().finite().unwrap();
        let at = |a: Q, b: Q| {
            let p = cast.frame().point(&a, &b);
            add3(&ap, &scale3(&sub3(&p, &ap), &Q::new(3, 2)))
        };
        assert_eq!(
            cast.contains(&at(Q::new(1, 2), Q::new(1, 2)), &edges),
            Some(true)
        );
        assert_eq!(cast.contains(&at(q(2), Q::new(5, 2)), &edges), Some(false)); // in the hole
        assert_eq!(cast.contains(&at(q(9), Q::new(1, 2)), &edges), Some(false));
    }

    /// The two views agree. On a convex profile — where "inside every wall" is a meaningful
    /// statement — the boundary view and the predicate view give the same answer at every sample.
    #[test]
    fn the_boundary_and_predicate_views_agree() {
        let cast = skew_cast();
        let edges = poly_edges(&[(0, 0), (3, 0), (0, 3)], 0);
        let walls = cast.walls(&edges).expect("three real walls");
        let ap = cast.apex().finite().unwrap();
        for ai in -2..=4 {
            for bi in -2..=4 {
                // Off every carrier: the two views agree on the open interior and the open
                // exterior, but not *on* a boundary line, where "inside" is a convention — strict
                // half-space intersection excludes it, even-odd parity counts it or not depending
                // on which side the ray leaves. Sampling the boundary would be testing the
                // convention, not the agreement.
                let (a, b) = (
                    Q::from_i128(ai).add(&Q::new(1, 5)),
                    Q::from_i128(bi).add(&Q::new(1, 3)),
                );
                let p = cast.frame().point(&a, &b);
                let x = add3(&ap, &scale3(&sub3(&p, &ap), &q(2)));
                // A CCW loop's interior is to the left of each directed edge, so "inside" is where
                // every wall residual is positive — each wall mirroring its own carrier.
                let by_walls = walls
                    .iter()
                    .all(|w| w.residual(&x).expect("well-formed").sign() > 0);
                assert_eq!(cast.contains(&x, &edges), Some(by_walls));
            }
        }
    }

    /// Pushing the cast point outward degrades the cone to the cylinder continuously — the API has
    /// no discontinuity at "parallel", because parallel is just `w = 0`.
    #[test]
    fn a_distant_cast_point_approaches_the_parallel_extrusion() {
        let (o, e1, e2) = (o3(), [q(1), q(0), q(0)], [q(0), q(1), q(0)]);
        // On the unit cylinder at height 1 — and *outside* every cone cast from above it, since a
        // cone tapers toward its apex. The further out the cast point, the less it tapers.
        let probe = [q(1), q(0), q(1)];
        let cyl = ellipse_wall(&o, &e1, &e2, &apex_dir(0, 0, 1)).expect("cylinder");
        assert!(res(&cyl, &probe).is_zero());
        let mut last: Option<Q> = None;
        for z in [10i128, 100, 1_000, 10_000] {
            // Normalize by the squared weight `k² = z²` so the residuals are comparable across
            // apexes — the surface is a zero set, so any positive rescaling is the same surface.
            let cone = ellipse_wall(&o, &e1, &e2, &apex_pt(0, 0, z)).expect("cone");
            let r = res(&cone, &probe).div(&q(z).mul(&q(z)));
            assert!(r.sign() > 0, "the probe is outside a cone tapering upward");
            if let Some(prev) = last {
                assert!(r.sub(&prev).sign() < 0, "the residual must fall toward 0");
            }
            last = Some(r);
        }
        // At a cast point 10⁴ away the cone and the cylinder agree here to ~2·10⁻⁴.
        assert!(last.unwrap().cmp(&Q::new(1, 1000)) == core::cmp::Ordering::Less);
    }
}
