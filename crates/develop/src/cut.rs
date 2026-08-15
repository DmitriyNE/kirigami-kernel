//! The **cut-fit certificate** — G2: certify that a proposed rational ruling-rail
//! `μ̂(σ)` traces a cutting surface `{F(X) = 0}` on the cone, so it can enter the
//! certified unroll/anchor pipeline as a genuine cut curve.
//!
//! A rail point `C(σ, μ̂(σ)) = pedal(σ) + μ̂(σ)·ruling(σ) + w·normal(σ)` is **on the
//! cone by construction**; the only obligation is that it also lies on the cut
//! *surface*. So the certificate is a **geometric-distance bound**
//! `sup_σ dist(C(σ, μ̂(σ)), {F=0}) ≤ ε`, gated by the DRC `ε < clearance/2`
//! (spec:192) — "on the surface ∧ on the cone ⟹ on the cut curve". It is the
//! rational sibling of [`crate::anchor::anchor_dev`]: same scaffolding (subdivide
//! the σ-span, interval-enclose, take `ε = max`, DRC), but the residual is
//! **purely rational in σ** — no `cos`/`sin`/`arctan` development — so it reuses
//! only the rational interval primitives, never the transcendental ones.
//!
//! Two surface kinds cover the Stage-1 cuts (the 3-D lift of
//! [`certify_core::cap_in`]'s `Carrier` line/circle): an **offset plane**
//! `{n·X = d}` — whose exact rail [`plane_cut_rail`] is *rational*, verified with
//! `ε ≈ 0` — and a **cylinder**, whose cone∩cylinder rail is a surd fitted by a
//! float oracle (`export::cut_oracle`) and re-verified here. Fail-closed: a loose
//! fit or wrong branch yields a large `ε` ⇒ [`Unresolved`](Verdict::Unresolved),
//! never a wrong [`Verified`](Verdict::Verified). No float enters this certificate.

use crate::cone::DevConfig;
use crate::interval::{ROUND_BITS, RatIv, abs_on, eval_ratfunc_on, sqrt, sqrt_on};
use certify_core::Verdict;
use geom::chart::Chart;
use lattice::{Backend, Bignum, Interval, Poly, Rat, RatFunc, Vec3Rat};

/// A rational cutting surface `{F(X) = 0}` — the 3-D lift of
/// [`certify_core::cap_in`]'s 2-D `Carrier` (line ↦ plane, circle ↦ cylinder).
#[derive(Clone)]
pub enum CutSurface<B: Backend = Bignum> {
    /// The plane `{n·X = d}`. `n` need not be unit — the certificate divides the
    /// implicit residual `n·X − d` by `|n|` to get a true geometric distance.
    Plane {
        /// The plane normal `n` (nonzero).
        n: [Rat<B>; 3],
        /// The plane offset `d`.
        d: Rat<B>,
    },
    /// The cylinder of radius `√r2` about the axis through `axis_point` in direction
    /// `axis_dir` (which need not be unit — the certificate divides by `axis_dir·axis_dir`).
    Cylinder {
        /// A point on the cylinder axis.
        axis_point: [Rat<B>; 3],
        /// The axis direction (nonzero).
        axis_dir: [Rat<B>; 3],
        /// The squared radius `R²` (positive).
        r2: Rat<B>,
    },
    /// A general quadric `{ Xᵀ M X + b·X + c = 0 }`, restricted to the [`Nappe`] half-space.
    ///
    /// This is the wall an extruded profile *arc* sweeps
    /// ([`ellipse_wall`](crate::extrude::ellipse_wall)): an elliptic **cone** under a finite cast
    /// point, an elliptic **cylinder** under a direction. Neither is in general one of the two
    /// special surfaces above — the cone over a circle from an apex *off* the circle's own axis is
    /// oblique, hence elliptic, and a circle authored in a non-orthonormal frame is an ellipse to
    /// begin with — so this variant carries the general degree-2 form rather than a metric
    /// parametrization it could not always represent.
    ///
    /// Only the **symmetric part** of `m` affects the surface, and the checker uses `M + Mᵀ` for the
    /// gradient, so an asymmetric `m` is harmless rather than a silent wrong answer.
    ///
    /// Unlike the two above, this surface has no closed-form distance: its certificate uses the
    /// first-order bound in [`cut_fit`], which is why it is the one arm that needs to know the
    /// clearance.
    Quadric(Box<Quadric<B>>),
}

/// The coefficients of a [`CutSurface::Quadric`]: `{ Xᵀ M X + b·X + c = 0 }` on one [`Nappe`].
///
/// Boxed inside the enum because it is three times the size of the two special surfaces, which are
/// what the existing pipelines pass around by value.
#[derive(Clone)]
pub struct Quadric<B: Backend = Bignum> {
    /// The quadratic coefficient matrix `M`.
    pub m: [[Rat<B>; 3]; 3],
    /// The linear coefficient `b`.
    pub b: [Rat<B>; 3],
    /// The constant coefficient `c`.
    pub c: Rat<B>,
    /// The nappe selector — which half of a double cone is the authored cutter.
    pub nappe: Nappe<B>,
}

/// The half-space `{ n·X > d }` that selects **one nappe** of a quadric cone.
///
/// A finite apex generates a *double* cone, and only the nappe on the authored side is the cutter;
/// without the selector a cut would reappear mirrored beyond the apex (`docs/cutter-extrude-design.md`
/// §4.1). A wall with no nappe to choose — a cylinder, whose apex is at infinity — carries the
/// **vacuous** selector `n = 0`, `d < 0`, so one formula covers both and no branch is needed.
///
/// The apex itself sits on the selector's boundary plane, so requiring the selector *strictly* also
/// discharges §4.1's second condition, apex clearance: a cut band that reaches the apex — where
/// "inside" inverts — fails the same test.
#[derive(Clone)]
pub struct Nappe<B: Backend = Bignum> {
    /// The selector normal, or `0` when there is no nappe to choose.
    pub n: [Rat<B>; 3],
    /// The selector offset.
    pub d: Rat<B>,
}

impl<B: Backend> CutSurface<B> {
    /// The implicit residual at a point — **negative strictly inside** the solid cutter, zero on the
    /// surface. `None` only for malformed surface data (a zero cylinder axis).
    ///
    /// This is the *predicate* view of a cut surface: it is exact, cheap, and it is what a
    /// containment or side test wants. The *boundary* view — where the cut runs on a sheet — is
    /// [`cut_mu_form`] instead.
    pub fn residual(&self, x: &[Rat<B>; 3]) -> Option<Rat<B>> {
        match self {
            CutSurface::Plane { n, d } => Some(dot3(n, x).sub(d)),
            CutSurface::Cylinder {
                axis_point,
                axis_dir,
                r2,
            } => {
                let a2 = dot3(axis_dir, axis_dir);
                if a2.sign() <= 0 {
                    return None;
                }
                let v = [
                    x[0].sub(&axis_point[0]),
                    x[1].sub(&axis_point[1]),
                    x[2].sub(&axis_point[2]),
                ];
                let av = dot3(&v, axis_dir);
                Some(dot3(&v, &v).sub(&av.mul(&av).div(&a2)).sub(r2))
            }
            CutSurface::Quadric(q) => {
                let mut acc = q.c.clone();
                for i in 0..3 {
                    acc = acc.add(&q.b[i].mul(&x[i]));
                    for j in 0..3 {
                        acc = acc.add(&q.m[i][j].mul(&x[i]).mul(&x[j]));
                    }
                }
                Some(acc)
            }
        }
    }

    /// Whether a point lies on the **authored** nappe. Always true for a surface with no nappe to
    /// choose; for a [`Quadric`](CutSurface::Quadric) it is the strict `n·X > d`.
    pub fn on_nappe(&self, x: &[Rat<B>; 3]) -> bool {
        match self {
            CutSurface::Quadric(q) => {
                dot3(&q.nappe.n, x).cmp(&q.nappe.d) == core::cmp::Ordering::Greater
            }
            _ => true,
        }
    }
}

/// A cut-fit certificate: a proposed ruling-rail `μ̂(σ)` (at layer offset `w`) that
/// claims to trace `surface` over the σ-`span`, checked against the fab `clearance`.
#[derive(Clone)]
pub struct CutFitCert<B: Backend = Bignum> {
    /// The proposed rail `μ̂(σ)` — the ruling coordinate as a rational function of σ
    /// (exact for a plane cut; a float-oracle fit for a cylinder cut).
    pub mu_hat: RatFunc<B>,
    /// The layer offset `w` along the normal (`0` for the single-layer mid-surface).
    pub w: Rat<B>,
    /// The cutting surface the rail claims to lie on.
    pub surface: CutSurface<B>,
    /// The σ-span `[σ_lo, σ_hi]` the rail is authored over.
    pub span: Interval<B>,
    /// The number of equal σ-sub-intervals the rigorous `sup_σ` is taken over — the
    /// refinement handle (more sub-intervals ⇒ a tighter `ε`).
    pub subdiv: usize,
    /// The item's exact fab clearance; the DRC gate is `ε < clearance/2`.
    pub clearance: Rat<B>,
    /// The `√`-bisection budget (`sqrt_eps`) for the radius / norm enclosures.
    pub cfg: DevConfig<B>,
}

/// The evidence a valid cut-fit carries: the certified σ-span and the uniform
/// distance bound `ε`, under the recorded clearance.
#[derive(Clone)]
pub struct ValidCutFit<B: Backend = Bignum> {
    /// The σ-span over which the bound holds.
    pub span: Interval<B>,
    /// The certified uniform bound `sup_σ dist(C(σ, μ̂(σ)), {F=0}) ≤ ε`.
    pub eps: Rat<B>,
    /// The clearance the DRC compared against (`ε < clearance/2`).
    pub clearance: Rat<B>,
}

/// Why the cut-fit checker refused a certificate (looseness is *not* here — a loose
/// fit is [`Unresolved`](Verdict::Unresolved), refined by `subdiv`, never `Refuted`).
#[derive(Clone, Debug)]
pub enum CutFitFault {
    /// The σ-span is empty or degenerate (`σ_lo ≥ σ_hi`).
    DegenerateSpan,
    /// The surface is malformed: a zero plane normal or zero cylinder axis direction.
    DegenerateSurface,
    /// A rational field (the rail `μ̂`, a chart field, or the residual) had a
    /// denominator enclosure straddling zero on a sub-interval — a possible pole, so
    /// the quotient is unbounded there. Refine, or re-author the span away from the pole.
    PoleInEval,
    /// The traced band is not strictly on the cutter's authored [`Nappe`]: it reaches the mirror
    /// nappe of a double cone, or it comes within the certificate's own working radius of the apex,
    /// where "inside" inverts. Both `docs/cutter-extrude-design.md` §4.1 conditions land here, and
    /// both are refusals — re-author the cut or move the cast point, never refine.
    NappeCrossed,
    /// A ruling meets the cutter in **more than one stretch** — a non-convex profile, or one with a
    /// hole of its own. [`shadow_cut_loop`] emits a *band* (one lower boundary, one upper), which
    /// cannot express that, so it refuses rather than picking a stretch and shipping a hole that is
    /// not the one drawn. `docs/cutter-extrude-design.md` §10.1.
    ShadowNotSimple,
    /// The cutter is not an **interior** hole over this window: it swallows a whole ruling, or its
    /// footprint reaches the window's own edge instead of closing inside it. Either way there is no
    /// closed loop to build here — widen the window, or author the cut as a boundary op.
    ShadowUnbounded,
    /// The cutter's own fill rule could not be read at a sample, even after the genericity nudge
    /// ([`Cast::contains`](crate::extrude::Cast::contains) returning `None` at every offset tried).
    /// A refusal, not a guess.
    ShadowUndecided,
}

/// A constant vector as a degree-0 [`Vec3Rat`] (denominator `1`), so it dots with the
/// chart's σ-rational fields. (Local copy of `closure::trim`'s helper — `develop`
/// does not depend on `closure`.)
fn const_vec3<B: Backend>(v: &[Rat<B>; 3]) -> Vec3Rat<B> {
    Vec3Rat::new(
        [
            Poly::constant(v[0].clone()),
            Poly::constant(v[1].clone()),
            Poly::constant(v[2].clone()),
        ],
        Poly::constant(Rat::from_i128(1)),
    )
}

/// The exact dot product of two constant 3-vectors.
fn dot3<B: Backend>(a: &[Rat<B>; 3], b: &[Rat<B>; 3]) -> Rat<B> {
    a[0].mul(&b[0]).add(&a[1].mul(&b[1])).add(&a[2].mul(&b[2]))
}

/// The **exact** offset-plane cut rail `μ(σ) = (d − n·pedal(σ)) / (n·ruling(σ))`, the
/// solution of `n·C(σ,μ) = d` (affine in μ). Rational — no fit — so [`cut_fit`]
/// verifies it with `ε ≈ 0`. The denominator `n·ruling(σ)` vanishes exactly where the
/// ruling is parallel to the plane (the cut exits the gore); keep the span clear of it.
pub fn plane_cut_rail<B: Backend>(chart: &Chart<B>, n: &[Rat<B>; 3], d: &Rat<B>) -> RatFunc<B> {
    let nv = const_vec3(n);
    let g0 = chart.pedal().dot(&nv); // n·pedal(σ)
    let g_mu = chart.ruling().dot(&nv); // n·ruling(σ)
    let num = RatFunc::from_poly(Poly::constant(d.clone())).sub(&g0); // d − n·pedal
    num.div(&g_mu)
}

/// The **µ̂-pullback** of a cut surface onto a chart: the implicit residual of the surface along
/// the ruling, `s(σ, µ̂) = a(σ)·µ̂² + b(σ)·µ̂ + c(σ)`, with σ-rational coefficients — every
/// [`CutSurface`] is degree ≤ 2 in µ̂ because the chart is ruled (`X = pedal + µ̂·ruling + w·normal`
/// is affine in µ̂). Reads the **true** chart fields, so it is correct on offset supports (`h ≠ 0`)
/// and under wrapping parametrizations — never an apex-ray shortcut. Built by [`cut_mu_form`].
///
/// Sign semantics: for a [`CutSurface::Cylinder`] the residual is `perp² − R²` — **negative
/// strictly inside** the solid cylinder; for a [`CutSurface::Plane`] it is `n·X − d` (`a ≡ 0`) —
/// negative on the `n·X < d` side. So `s < 0` is the natural "inside the solid cutter" predicate.
pub struct MuCut<B: Backend = Bignum> {
    /// The µ̂² coefficient (`0` for a plane; `≥ 0` pointwise for a cylinder, by Cauchy–Schwarz).
    pub a: RatFunc<B>,
    /// The µ̂ coefficient.
    pub b: RatFunc<B>,
    /// The µ̂⁰ term.
    pub c: RatFunc<B>,
}

impl<B: Backend> MuCut<B> {
    /// The residual `s(σ, µ̂)` at a rational point, or `None` on a coefficient pole.
    pub fn eval(&self, sigma: &Rat<B>, mu_hat: &Rat<B>) -> Option<Rat<B>> {
        let a = self.a.eval(sigma)?;
        let b = self.b.eval(sigma)?;
        let c = self.c.eval(sigma)?;
        Some(a.mul(mu_hat).add(&b).mul(mu_hat).add(&c))
    }

    /// The discriminant `b² − 4ac` — for a cylinder cut, **positive exactly where the ruling
    /// crosses the cylinder** (two real µ̂ branches); its roots are the true tangent rulings, the
    /// σ-extent of an interior hole. (For a plane, `a ≡ 0` and this degenerates to `b²`.)
    pub fn disc(&self) -> RatFunc<B> {
        self.b.mul(&self.b).sub(
            &self
                .a
                .mul(&self.c)
                .mul(&RatFunc::from_poly(Poly::constant(Rat::from_i128(4)))),
        )
    }
}

/// A closed **cut loop** in the domain: the pieces of a solid cutter's intersection with the
/// sheet, in traversal order, with the certified bounds that make it usable.
pub struct CutLoop<B: Backend = Bignum> {
    /// The closed loop's pieces, head-to-tail in traversal order.
    pub pieces: Vec<crate::pcurve::PCurve<B>>,
    /// The certified `sup dist(·, {F=0})` over every piece — how far the emitted loop can lie
    /// from the true cut.
    pub eps: Rat<B>,
    /// The half-width still open at the two extreme vertices: the loop's endpoints sit at the
    /// *midline* of a ruling just inside the window, so the two branches meet at a single vertex
    /// rather than being bridged, and this is the certified distance from that vertex to the true
    /// tangent point. It is **included in `eps`** — no unaccounted residual.
    pub tangent_gap: Rat<B>,
}

impl<B: Backend> MuCut<B> {
    /// The branch data at a ruling: the **midline** `m = −b/2a` (exact) and the **half-width**
    /// `h = √(b²−4ac)/2a` (a surd, returned as a rational inside its certified enclosure). The two
    /// cut points on the ruling are `m ± h`, and they coincide exactly where `h = 0` — the tangent
    /// rulings that bound the window. `None` off the cut (`h²< 0`), at a pole, or where `a`
    /// vanishes (the quadratic degenerates and one branch escapes to infinity).
    pub fn branch_at(&self, sigma: &Rat<B>, sqrt_eps: &Rat<B>) -> Option<(Rat<B>, Rat<B>)> {
        let a = self.a.eval(sigma)?;
        if a.sign() == 0 {
            return None;
        }
        let b = self.b.eval(sigma)?;
        let c = self.c.eval(sigma)?;
        let two_a = a.mul(&Rat::from_i128(2));
        let m = Rat::from_i128(0).sub(&b).div(&two_a);
        let disc = b.mul(&b).sub(&a.mul(&c).mul(&Rat::from_i128(4)));
        if disc.sign() < 0 {
            return None;
        }
        let h = sqrt(&disc, sqrt_eps).mid().div(&abs_rat(&two_a));
        Some((m, h))
    }

    /// Where the ruling at `sigma` **crosses** this cut: 0, 1 or 2 µ̂ values in increasing order,
    /// each flagged `true` for the upper root. `None` only on a coefficient pole.
    ///
    /// [`branch_at`](Self::branch_at) is the two-root special case, and deliberately returns `None`
    /// where `a` vanishes — a genuine quadric window has no such σ, and one branch escaping to
    /// infinity there is not a window end. A **wall** of an extruded profile has no such guarantee:
    /// the wall of a straight profile edge is a plane, whose pullback is affine at *every* σ and
    /// crosses the ruling exactly once. Both cases are ordinary here, and the empty return
    /// (`a = b = 0`, or a negative discriminant) means the ruling misses this wall — not a fault,
    /// since the profile's own fill rule decides what that implies.
    pub fn roots_at(&self, sigma: &Rat<B>, sqrt_eps: &Rat<B>) -> Option<Vec<(Rat<B>, bool)>> {
        let a = self.a.eval(sigma)?;
        let b = self.b.eval(sigma)?;
        let c = self.c.eval(sigma)?;
        // Exact, not a tolerance: the coefficients are rationals here, so `a = 0` is decidable.
        // (The resolver's float mirror of this needs a tolerance; this one must not have one.)
        if a.sign() == 0 {
            if b.sign() == 0 {
                return Some(Vec::new());
            }
            return Some(vec![(c.neg().div(&b), false)]);
        }
        let disc = b.mul(&b).sub(&a.mul(&c).mul(&Rat::from_i128(4)));
        if disc.sign() < 0 {
            return Some(Vec::new());
        }
        let two_a = a.mul(&Rat::from_i128(2));
        let m = Rat::from_i128(0).sub(&b).div(&two_a);
        let h = sqrt(&disc, sqrt_eps).mid().div(&abs_rat(&two_a));
        Some(vec![(m.sub(&h), false), (m.add(&h), true)])
    }
}

/// `|r|`.
fn abs_rat<B: Backend>(r: &Rat<B>) -> Rat<B> {
    if r.sign() < 0 {
        Rat::from_i128(0).sub(r)
    } else {
        r.clone()
    }
}

/// The straight domain segment between two `(σ, µ̂)` points, as a p-curve over `t ∈ [0, 1]`.
fn segment<B: Backend>(a: &(Rat<B>, Rat<B>), b: &(Rat<B>, Rat<B>)) -> crate::pcurve::PCurve<B> {
    let lin =
        |p: &Rat<B>, q: &Rat<B>| RatFunc::from_poly(Poly::from_coeffs(vec![p.clone(), q.sub(p)]));
    crate::pcurve::PCurve {
        sigma: lin(&a.0, &b.0),
        mu: lin(&a.1, &b.1),
        domain: Interval {
            lo: Rat::from_i128(0),
            hi: Rat::from_i128(1),
        },
    }
}

/// Build the **closed cut loop** of a solid quadric cutter over one of its σ-windows — the
/// interior-hole boundary, as a p-curve loop that passes *through* both tangent rulings.
///
/// The cut is a µ̂-quadratic ([`MuCut`]), so on each ruling it is the pair `m(σ) ± h(σ)` — midline
/// exact, half-width a surd vanishing at the window's two tangent rulings. Rather than fit two
/// graphs `µ̂ = f(σ)` (which cannot reach a vertical tangent, so they must stop short and be
/// bridged by a straight chord ~30% of the hole across at best), this walks the true branches to
/// their meeting points: the loop's extreme vertices sit on the **midline** of a ruling just
/// inside the window, where the two branches differ by `tangent_gap`, driven to the bisected
/// root's own resolution rather than to a fit's reach.
///
/// Nodes are **√-graded** toward each tangent — the branch behaves like `µ̂ − µ̂_t ∝ √(σ − σ_t)`
/// there, so nodes uniform in `√(σ − σ_t)` land uniformly along the curve instead of bunching in
/// σ where the curve is turning hardest.
///
/// Every piece is certified against the true cutter surface by [`pcurve_cut_fit`], so the emitted
/// loop's distance to the real cut is bounded whatever the node placement — the grading buys
/// tightness, never soundness. Returns `Unresolved(ε)` if the loop is too coarse for the
/// clearance (refine `segments`), `Refuted` for a degenerate window or a cut that is not a proper
/// two-branch window.
#[allow(clippy::too_many_arguments)]
pub fn quadric_cut_loop<B: Backend>(
    chart: &Chart<B>,
    surface: &CutSurface<B>,
    window: &Interval<B>,
    w: &Rat<B>,
    segments: usize,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Verdict<CutLoop<B>, CutFitFault, Rat<B>> {
    use core::cmp::Ordering;
    if window.lo.cmp(&window.hi) != Ordering::Less {
        return Verdict::Refuted(CutFitFault::DegenerateSpan);
    }
    let mc = match cut_mu_form(chart, surface, w) {
        Some(m) => m,
        None => return Verdict::Refuted(CutFitFault::DegenerateSurface),
    };
    let n = segments.max(2);
    // Every emitted coordinate is snapped to this dyadic grid. The window ends are bisected
    // roots and the branch values are surds, so unsnapped vertices carry hundreds of digits and
    // the residual polynomials built from them stop being evaluable — the enclosure of a
    // positive denominator straddles zero. Snapping is safe precisely because each piece is
    // certified against the true surface afterwards.
    const BITS: u32 = 30;
    /// How many grid steps an end may be walked inward before the window is judged unreal.
    const MAX_NUDGE: usize = 64;
    /// Sub-intervals per piece for the per-piece certificate. The p-curve bound is first-order in
    /// this (see [`pcurve_cut_fit`]), and the pieces nearest a tangent sweep the most µ̂ per unit
    /// σ, so a coarse setting here — not the geometry — is what makes a loop read loose.
    const PIECE_SUBDIV: usize = 64;
    let lo = crate::pcurve::snap(&window.lo, BITS);
    let hi = crate::pcurve::snap(&window.hi, BITS);
    if lo.cmp(&hi) != Ordering::Less {
        return Verdict::Refuted(CutFitFault::DegenerateSpan);
    }
    let window = &Interval { lo, hi };
    let half = window.hi.sub(&window.lo).mul(&Rat::new(1, 2));

    // √-graded nodes: σ = end ± (k/n)²·half, so the spacing collapses toward each tangent at the
    // same rate the branch's slope blows up.
    let graded = |k: usize| -> Rat<B> {
        let f = Rat::new(k as i128, n as i128);
        crate::pcurve::snap(&f.mul(&f).mul(&half), BITS)
    };
    let mut nodes: Vec<Rat<B>> = Vec::with_capacity(2 * n + 1);
    let push = |s: Rat<B>, into: &mut Vec<Rat<B>>| {
        if into
            .last()
            .map(|p| p.cmp(&s) != Ordering::Equal)
            .unwrap_or(true)
        {
            into.push(s);
        }
    };
    for k in 0..=n {
        push(window.lo.add(&graded(k)), &mut nodes);
    }
    for k in (0..n).rev() {
        push(window.hi.sub(&graded(k)), &mut nodes);
    }
    if nodes.len() < 3 {
        return Verdict::Refuted(CutFitFault::DegenerateSpan);
    }

    // The extreme vertices ride the midline (both branches meet there); the interior vertices
    // ride the two branches. A node whose ruling misses the cut is skipped — the window's ends
    // are bisected roots, so the very first/last ruling can fall a hair outside.
    let branch = |s: &Rat<B>| {
        mc.branch_at(s, &cfg.sqrt_eps)
            .map(|(m, h)| (crate::pcurve::snap(&m, BITS), crate::pcurve::snap(&h, BITS)))
    };
    // The window ends are bisected roots snapped to the grid, so an end can land a grid step
    // *outside* the cut (discriminant negative, no real ruling intersection). Walk inward one
    // grid step at a time until the cut is real: the tangent vertex then sits at most a few
    // 2^-30 from the true tangent ruling, and `tangent_gap` records exactly how far.
    let unit = Rat::new(1, 1i128 << BITS);
    let step_in = |from: &Rat<B>, inward: bool| -> Option<(Rat<B>, Rat<B>, Rat<B>)> {
        let mut s = from.clone();
        for _ in 0..MAX_NUDGE {
            if let Some((m, h)) = branch(&s) {
                return Some((s, m, h));
            }
            s = if inward { s.add(&unit) } else { s.sub(&unit) };
        }
        None
    };
    let (first, last) = (nodes[0].clone(), nodes[nodes.len() - 1].clone());
    let mut tangent_gap = Rat::from_i128(0);
    let mut ends: Vec<(Rat<B>, Rat<B>)> = Vec::new();
    for (s, inward) in [(&first, true), (&last, false)] {
        match step_in(s, inward) {
            Some((s, m, h)) => {
                if h.cmp(&tangent_gap) == Ordering::Greater {
                    tangent_gap = h;
                }
                ends.push((s, m));
            }
            None => return Verdict::Refuted(CutFitFault::DegenerateSurface),
        }
    }
    let mut far: Vec<(Rat<B>, Rat<B>)> = Vec::new();
    let mut near: Vec<(Rat<B>, Rat<B>)> = Vec::new();
    for s in &nodes[1..nodes.len() - 1] {
        if let Some((m, h)) = branch(s) {
            far.push((s.clone(), m.add(&h)));
            near.push((s.clone(), m.sub(&h)));
        }
    }
    if far.is_empty() {
        return Verdict::Refuted(CutFitFault::DegenerateSpan);
    }

    // One closed traversal: left tangent → far branch → right tangent → near branch back.
    let mut loop_pts: Vec<(Rat<B>, Rat<B>)> = Vec::with_capacity(2 * far.len() + 2);
    loop_pts.push(ends[0].clone());
    loop_pts.extend(far);
    loop_pts.push(ends[1].clone());
    near.reverse();
    loop_pts.extend(near);

    let mut pieces = Vec::with_capacity(loop_pts.len());
    let mut eps = tangent_gap.clone();
    for k in 0..loop_pts.len() {
        let piece = segment(&loop_pts[k], &loop_pts[(k + 1) % loop_pts.len()]);
        match pcurve_cut_fit(chart, &piece, surface, w, PIECE_SUBDIV, clearance, cfg) {
            Verdict::Verified(v) => {
                if v.eps.cmp(&eps) == Ordering::Greater {
                    eps = v.eps;
                }
            }
            Verdict::Unresolved(e) => {
                if e.cmp(&eps) == Ordering::Greater {
                    eps = e;
                }
            }
            Verdict::Refuted(f) => return Verdict::Refuted(f),
        }
        pieces.push(piece);
    }
    // Round the accumulated bound up onto the grid: sound (a larger upper bound is still one)
    // and necessary, since interval arithmetic over snapped surds grows thousand-digit rationals.
    let eps = crate::pcurve::snap_up(&eps, BITS);
    let drc = clearance.mul(&Rat::new(1, 2));
    if eps.cmp(&drc) == Ordering::Less {
        Verdict::Verified(CutLoop {
            pieces,
            eps,
            tangent_gap,
        })
    } else {
        Verdict::Unresolved(eps)
    }
}

/// Which wall of a multi-walled cutter, and which of its roots, produced a µ̂ value:
/// `(wall index, upper root)`. The mirror of the resolver's `BranchSide::Wall`.
pub type WallRoot = (usize, bool);

/// One stretch of a ruling lying **inside** a cutter whose boundary is several walls: the µ̂
/// interval `[lo, hi]`, each end tagged with the wall and root that produced it.
pub struct RulingPatch<B: Backend = Bignum> {
    /// The stretch's lower µ̂ end.
    pub lo: Rat<B>,
    /// The stretch's upper µ̂ end.
    pub hi: Rat<B>,
    /// Which wall and root gives [`lo`](Self::lo).
    pub lo_at: WallRoot,
    /// Which wall and root gives [`hi`](Self::hi).
    pub hi_at: WallRoot,
}

/// The ruling at `sigma`, intersected with the solid whose **boundary** is `forms` (one µ̂-form per
/// wall) and whose **inside** is `inside(σ, µ̂)` — the two-view split of
/// `docs/cutter-extrude-design.md` §2.1, read along one ruling.
///
/// Every wall contributes its crossings; sorted, they cut the ruling into stretches, each wholly
/// inside the solid or wholly outside, so one membership test per stretch classifies it exactly.
/// This is the exact sibling of the resolver's float `extruded_shadow` — same construction, but the
/// crossings are rationals rather than `f64`, because what is built from them is emitted geometry
/// rather than a structural decision.
///
/// `Ok(None)` when the ruling misses the solid. Refuses rather than guessing: several inside
/// stretches are [`ShadowNotSimple`](CutFitFault::ShadowNotSimple), an inside stretch running to
/// infinity is [`ShadowUnbounded`](CutFitFault::ShadowUnbounded), an unreadable fill is
/// [`ShadowUndecided`](CutFitFault::ShadowUndecided).
fn ruling_patch<B: Backend, F>(
    forms: &[MuCut<B>],
    sigma: &Rat<B>,
    inside: &F,
    sqrt_eps: &Rat<B>,
) -> Result<Option<RulingPatch<B>>, CutFitFault>
where
    F: Fn(&Rat<B>, &Rat<B>) -> Option<bool>,
{
    use core::cmp::Ordering;
    let mut cuts: Vec<(Rat<B>, WallRoot)> = Vec::new();
    for (wi, form) in forms.iter().enumerate() {
        let roots = form
            .roots_at(sigma, sqrt_eps)
            .ok_or(CutFitFault::PoleInEval)?;
        for (mu, upper) in roots {
            cuts.push((mu, (wi, upper)));
        }
    }
    cuts.sort_by(|x, y| x.0.cmp(&y.0));

    // Membership is constant *between* consecutive crossings — that is what makes one midpoint
    // sample exact — so the genericity nudge must stay inside the stretch being classified, which
    // is why `scale` is the stretch's own width and never a global one.
    let decide = |mu: &Rat<B>, scale: &Rat<B>| -> Result<bool, CutFitFault> {
        for k in 0..4i128 {
            let sign = if k % 2 == 0 { 1 } else { -1 };
            let jitter = scale.mul(&Rat::new(sign * k, 1000));
            if let Some(v) = inside(sigma, &mu.add(&jitter)) {
                return Ok(v);
            }
        }
        Err(CutFitFault::ShadowUndecided)
    };

    let one = Rat::from_i128(1);
    if cuts.is_empty() {
        // No wall meets this ruling: it is wholly inside the solid or wholly outside.
        return if decide(&Rat::from_i128(0), &one)? {
            Err(CutFitFault::ShadowUnbounded)
        } else {
            Ok(None)
        };
    }
    let span = cuts[cuts.len() - 1].0.sub(&cuts[0].0);
    let reach = if span.cmp(&one) == Ordering::Greater {
        span
    } else {
        one
    };
    // The two unbounded stretches: inside either of them and this is not an interior hole.
    for (end, away) in [
        (&cuts[0].0, reach.neg()),
        (&cuts[cuts.len() - 1].0, reach.clone()),
    ] {
        if decide(&end.add(&away), &reach)? {
            return Err(CutFitFault::ShadowUnbounded);
        }
    }
    let mut found: Option<RulingPatch<B>> = None;
    for pair in cuts.windows(2) {
        let (lo, hi) = (&pair[0], &pair[1]);
        let width = hi.0.sub(&lo.0);
        if width.sign() <= 0 {
            continue;
        }
        if !decide(&lo.0.add(&hi.0).mul(&Rat::new(1, 2)), &width)? {
            continue;
        }
        if found.is_some() {
            return Err(CutFitFault::ShadowNotSimple);
        }
        found = Some(RulingPatch {
            lo: lo.0.clone(),
            hi: hi.0.clone(),
            lo_at: lo.1,
            hi_at: hi.1,
        });
    }
    Ok(found)
}

/// One vertex of a multi-wall loop's boundary: the ruling, both of its ends, and which wall bounds
/// it on each side.
struct BandNode<B: Backend> {
    sigma: Rat<B>,
    lo: Rat<B>,
    hi: Rat<B>,
    lo_at: WallRoot,
    hi_at: WallRoot,
}

/// Build the **closed cut loop** of a cutter bounded by *several* walls — the interior-hole
/// boundary of an extruded profile that is a polygon, a rounded slot, or any other multi-carrier
/// outline (`docs/cutter-extrude-design.md` §10).
///
/// [`quadric_cut_loop`] reads the two branches `m(σ) ± h(σ)` off **one** µ̂-quadratic, which is
/// exactly what a multi-walled cutter does not have. Here each ruling's two boundary points come
/// from [`ruling_patch`] — every wall's crossings, classified by the profile's own fill rule — so
/// the wall governing the boundary is whichever one the fill rule selects, and it *changes along
/// the loop*, at every profile corner. Three things follow, and they are the whole design:
///
/// 1. **The window is found, not given.** `window` may be a superset (an all-affine profile has no
///    tangent-ruling window of its own, so station targeting hands over its *bounding circle's* —
///    §6). The true σ-extent is where the patch vanishes, bisected from a scan.
/// 2. **Corners get their own nodes.** Where the governing wall changes, the boundary has a kink
///    that no single wall's rail follows. The crossing σ is bisected and *two* nodes a grid step
///    apart are inserted, so the kink is spanned by one grid-step bridge rather than by a chord
///    across a whole node interval.
/// 3. **Each piece is certified against the wall its own endpoints name**, and a piece whose ends
///    disagree — the corner bridges — is certified against **both**, taking the larger bound.
///
/// Soundness never rests on the corner search: on top of the per-piece
/// [`pcurve_cut_fit`] bound, every piece is compared at its own σ-midpoint against the *true*
/// boundary the fill rule reports there, and the deviation folded into `eps`. A corner the search
/// missed (two of them inside one node interval, say) shows up as a loose `ε` and a
/// [`Unresolved`](Verdict::Unresolved) — refine `segments` — never as a quietly wrong hole.
///
/// **Scope, refused rather than approximated.** The loop is a *band*: one lower boundary, one
/// upper. A ruling that meets the cutter in several stretches — a non-convex profile, or one with
/// its own hole — is [`ShadowNotSimple`](CutFitFault::ShadowNotSimple). Those footprints are not
/// bands and want holes to be regions end-to-end, through to the B-rep builder.
#[allow(clippy::too_many_arguments)]
pub fn shadow_cut_loop<B: Backend, F>(
    chart: &Chart<B>,
    walls: &[CutSurface<B>],
    inside: F,
    window: &Interval<B>,
    w: &Rat<B>,
    segments: usize,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Verdict<CutLoop<B>, CutFitFault, Rat<B>>
where
    F: Fn(&Rat<B>, &Rat<B>) -> Option<bool>,
{
    use core::cmp::Ordering;
    match shadow_loop_inner(chart, walls, &inside, window, w, segments, clearance, cfg) {
        Err(f) => Verdict::Refuted(f),
        Ok(l) => {
            let drc = clearance.mul(&Rat::new(1, 2));
            if l.eps.cmp(&drc) == Ordering::Less {
                Verdict::Verified(l)
            } else {
                Verdict::Unresolved(l.eps)
            }
        }
    }
}

/// [`shadow_cut_loop`]'s body, in `Result` form so the refusals read as `?`.
#[allow(clippy::too_many_arguments)]
fn shadow_loop_inner<B: Backend, F>(
    chart: &Chart<B>,
    walls: &[CutSurface<B>],
    inside: &F,
    window: &Interval<B>,
    w: &Rat<B>,
    segments: usize,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Result<CutLoop<B>, CutFitFault>
where
    F: Fn(&Rat<B>, &Rat<B>) -> Option<bool>,
{
    use crate::pcurve::snap;
    use core::cmp::Ordering;
    /// The dyadic grid every emitted coordinate is snapped to (see [`quadric_cut_loop`]).
    const BITS: u32 = 30;
    /// Sub-intervals per piece for the per-piece certificate.
    const PIECE_SUBDIV: usize = 64;
    /// Bisection steps for a window end and for a corner — well past the `BITS` grid.
    const BISECT: usize = 32;
    /// How many corner-insertion sweeps to run (a sweep resolves one corner per node interval).
    const CORNER_SWEEPS: usize = 3;
    /// How many grid steps an end may be walked inward before the window is judged unreal.
    const MAX_NUDGE: usize = 64;

    if window.lo.cmp(&window.hi) != Ordering::Less {
        return Err(CutFitFault::DegenerateSpan);
    }
    if walls.is_empty() {
        return Err(CutFitFault::DegenerateSurface);
    }
    let mut forms = Vec::with_capacity(walls.len());
    for wall in walls {
        forms.push(cut_mu_form(chart, wall, w).ok_or(CutFitFault::DegenerateSurface)?);
    }
    let patch = |s: &Rat<B>| ruling_patch(&forms, s, inside, &cfg.sqrt_eps);
    let n = segments.max(2);
    let unit = Rat::new(1, 1i128 << BITS);
    let half_of = |a: &Rat<B>, b: &Rat<B>| a.add(b).mul(&Rat::new(1, 2));

    // — 1. The footprint's σ-extent inside the (possibly oversized) window. —
    let scan = (4 * n).max(48);
    let width = window.hi.sub(&window.lo);
    let at = |k: usize| {
        window
            .lo
            .add(&width.mul(&Rat::new(k as i128, scan as i128)))
    };
    let (mut first, mut last) = (None, None);
    for k in 0..=scan {
        if patch(&at(k))?.is_some() {
            first.get_or_insert(k);
            last = Some(k);
        }
    }
    let (first, last) = match (first, last) {
        (Some(f), Some(l)) => (f, l),
        _ => return Err(CutFitFault::DegenerateSpan),
    };
    // A footprint touching the window's own edge has no closing vertex there.
    if first == 0 || last == scan {
        return Err(CutFitFault::ShadowUnbounded);
    }
    // One band means one σ-window: a gap between the ends is a second window, not a corner.
    for k in first..=last {
        if patch(&at(k))?.is_none() {
            return Err(CutFitFault::ShadowNotSimple);
        }
    }
    // Bisect each end. `out` has no patch, `inn` does; the inside witness is kept.
    let edge = |out: Rat<B>, inn: Rat<B>| -> Result<Rat<B>, CutFitFault> {
        let (mut out, mut inn) = (out, inn);
        for _ in 0..BISECT {
            let m = half_of(&out, &inn);
            if patch(&m)?.is_some() {
                inn = m;
            } else {
                out = m;
            }
        }
        Ok(inn)
    };
    let e_lo = edge(at(first - 1), at(first))?;
    let e_hi = edge(at(last + 1), at(last))?;
    // Snapping an end to the grid can push it a step outside; walk back in, exactly as the
    // single-surface loop does with its bisected tangent rulings.
    let step_in = |from: &Rat<B>, sign: i128| -> Result<(Rat<B>, RulingPatch<B>), CutFitFault> {
        let mut s = snap(from, BITS);
        for _ in 0..MAX_NUDGE {
            if let Some(p) = patch(&s)? {
                return Ok((s, p));
            }
            s = s.add(&unit.mul(&Rat::from_i128(sign)));
        }
        Err(CutFitFault::DegenerateSpan)
    };
    let (s_lo, p_lo) = step_in(&e_lo, 1)?;
    let (s_hi, p_hi) = step_in(&e_hi, -1)?;
    if s_lo.cmp(&s_hi) != Ordering::Less {
        return Err(CutFitFault::DegenerateSpan);
    }
    // The loop closes to a single vertex at each end, on the patch's midline; this is how far that
    // vertex can be from the true pinch, and it is inside `eps`.
    let mut tangent_gap = Rat::from_i128(0);
    for p in [&p_lo, &p_hi] {
        let h = p.hi.sub(&p.lo).mul(&Rat::new(1, 2));
        if h.cmp(&tangent_gap) == Ordering::Greater {
            tangent_gap = h;
        }
    }
    let end_lo = (s_lo.clone(), snap(&half_of(&p_lo.lo, &p_lo.hi), BITS));
    let end_hi = (s_hi.clone(), snap(&half_of(&p_hi.lo, &p_hi.hi), BITS));

    // — 2. √-graded interior nodes, as in [`quadric_cut_loop`]: uniform in √(σ − σ_end), so they
    //   land uniformly along a branch that turns like a square root at the pinch. —
    let half = s_hi.sub(&s_lo).mul(&Rat::new(1, 2));
    let graded = |k: usize| -> Rat<B> {
        let f = Rat::new(k as i128, n as i128);
        snap(&f.mul(&f).mul(&half), BITS)
    };
    let mut sigmas: Vec<Rat<B>> = Vec::with_capacity(2 * n);
    let push = |s: Rat<B>, into: &mut Vec<Rat<B>>| {
        if into
            .last()
            .map(|p: &Rat<B>| p.cmp(&s) != Ordering::Equal)
            .unwrap_or(true)
        {
            into.push(s);
        }
    };
    for k in 1..n {
        push(s_lo.add(&graded(k)), &mut sigmas);
    }
    for k in (1..n).rev() {
        push(s_hi.sub(&graded(k)), &mut sigmas);
    }
    let mut nodes: Vec<BandNode<B>> = Vec::with_capacity(sigmas.len());
    for s in sigmas {
        // Strictly inside the two pinch vertices. On a narrow window the first graded offset can
        // snap to zero, which would put a node *on* an end and leave a vertical piece there — and a
        // vertical piece is not a rail, so the loop the solid builder reads would be open.
        if s.cmp(&s_lo) != Ordering::Greater || s.cmp(&s_hi) != Ordering::Less {
            continue;
        }
        if let Some(p) = patch(&s)? {
            nodes.push(BandNode {
                sigma: s,
                lo: snap(&p.lo, BITS),
                hi: snap(&p.hi, BITS),
                lo_at: p.lo_at,
                hi_at: p.hi_at,
            });
        }
    }
    if nodes.is_empty() {
        return Err(CutFitFault::DegenerateSpan);
    }

    // — 3. Corners: bisect every governing-wall change and bracket it with two grid-adjacent
    //   nodes, so the kink is spanned by one grid step rather than by a whole node interval. —
    let four = unit.mul(&Rat::from_i128(4));
    for _ in 0..CORNER_SWEEPS {
        let mut extra: Vec<Rat<B>> = Vec::new();
        for pair in nodes.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let upper = a.hi_at.0 != b.hi_at.0;
            if !upper && a.lo_at.0 == b.lo_at.0 {
                continue;
            }
            if b.sigma.sub(&a.sigma).cmp(&four) != Ordering::Greater {
                continue; // already bracketed to the grid
            }
            let want = if upper { a.hi_at.0 } else { a.lo_at.0 };
            let (mut l, mut r) = (a.sigma.clone(), b.sigma.clone());
            for _ in 0..BISECT {
                let m = half_of(&l, &r);
                match patch(&m)? {
                    Some(p) if (if upper { p.hi_at.0 } else { p.lo_at.0 }) == want => l = m,
                    _ => r = m,
                }
            }
            let c = snap(&l, BITS);
            extra.push(c.add(&unit));
            extra.push(c);
        }
        if extra.is_empty() {
            break;
        }
        for s in extra {
            if s.cmp(&s_lo) != Ordering::Greater || s.cmp(&s_hi) != Ordering::Less {
                continue;
            }
            if let Some(p) = patch(&s)? {
                nodes.push(BandNode {
                    sigma: s,
                    lo: snap(&p.lo, BITS),
                    hi: snap(&p.hi, BITS),
                    lo_at: p.lo_at,
                    hi_at: p.hi_at,
                });
            }
        }
        nodes.sort_by(|x, y| x.sigma.cmp(&y.sigma));
        nodes.dedup_by(|x, y| x.sigma.cmp(&y.sigma) == Ordering::Equal);
    }

    // — 4. The two chains, then one closed traversal: low pinch → upper boundary → high pinch →
    //   lower boundary back. A vertex's `WallRoot` is the wall its piece is certified against; the
    //   two pinch vertices belong to both boundaries and name none. —
    type Vertex<B> = (Rat<B>, Rat<B>, Option<WallRoot>);
    let mut upper: Vec<Vertex<B>> = Vec::with_capacity(nodes.len() + 2);
    let mut lower: Vec<Vertex<B>> = Vec::with_capacity(nodes.len() + 2);
    upper.push((end_lo.0.clone(), end_lo.1.clone(), None));
    for nd in &nodes {
        upper.push((nd.sigma.clone(), nd.hi.clone(), Some(nd.hi_at)));
    }
    upper.push((end_hi.0.clone(), end_hi.1.clone(), None));
    lower.push((end_hi.0.clone(), end_hi.1.clone(), None));
    for nd in nodes.iter().rev() {
        lower.push((nd.sigma.clone(), nd.lo.clone(), Some(nd.lo_at)));
    }
    lower.push((end_lo.0.clone(), end_lo.1.clone(), None));

    let mut eps = tangent_gap.clone();
    let mut pieces = Vec::with_capacity(upper.len() + lower.len());
    for (chain, is_upper) in [(&upper, true), (&lower, false)] {
        for pair in chain.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            // Nodes are deduped and kept strictly inside the two pinch vertices, so consecutive
            // σ differ by construction. Refuse rather than skip if that ever fails: a dropped
            // piece leaves the loop **open**, which the flat boolean would happily stitch into
            // something else and the solid builder would reject much further downstream.
            if a.0.cmp(&b.0) == Ordering::Equal {
                return Err(CutFitFault::DegenerateSpan);
            }
            let piece = segment(&(a.0.clone(), a.1.clone()), &(b.0.clone(), b.1.clone()));
            // Certify against every wall the piece's own endpoints name. Endpoints that disagree
            // are the corner bridges: both walls, larger bound — never a silent choice.
            let mut targets: Vec<usize> = Vec::with_capacity(2);
            for v in [a, b] {
                if let Some((wi, _)) = v.2
                    && !targets.contains(&wi)
                {
                    targets.push(wi);
                }
            }
            for wi in targets {
                match pcurve_cut_fit(chart, &piece, &walls[wi], w, PIECE_SUBDIV, clearance, cfg) {
                    Verdict::Verified(v) => {
                        if v.eps.cmp(&eps) == Ordering::Greater {
                            eps = v.eps;
                        }
                    }
                    Verdict::Unresolved(e) => {
                        if e.cmp(&eps) == Ordering::Greater {
                            eps = e;
                        }
                    }
                    Verdict::Refuted(f) => return Err(f),
                }
            }
            // The piece is on *a* wall; that it is on the **boundary** is a separate claim, and
            // this is what checks it: at the piece's own σ-midpoint, compare the emitted chord
            // against the boundary the fill rule reports there. A corner the bisection missed
            // lands here as a loose ε rather than as a hole that is quietly the wrong shape.
            let sm = half_of(&a.0, &b.0);
            if let Some(p) = patch(&sm)? {
                let truth = if is_upper { &p.hi } else { &p.lo };
                let dev = {
                    let d = half_of(&a.1, &b.1).sub(truth);
                    abs_rat(&d)
                };
                if dev.cmp(&eps) == Ordering::Greater {
                    eps = dev;
                }
            }
            pieces.push(piece);
        }
    }
    if pieces.len() < 3 {
        return Err(CutFitFault::DegenerateSpan);
    }
    Ok(CutLoop {
        pieces,
        eps: crate::pcurve::snap_up(&eps, BITS),
        tangent_gap,
    })
}

/// Pull a [`CutSurface`] back to its µ̂-form [`MuCut`] on `chart` at layer offset `w` (the
/// residual along `X(σ, µ̂) = pedal + µ̂·ruling + w·normal`). `None` for a degenerate surface
/// (zero plane normal / zero cylinder axis).
///
/// This is the single pedal-general pullback the trim/authoring layer composes on: plane rails
/// come from the linear root (`µ̂ = −c/b`, see [`plane_cut_rail`]), cylinder branches from the
/// quadratic roots, hole σ-extents from [`MuCut::disc`].
pub fn cut_mu_form<B: Backend>(
    chart: &Chart<B>,
    surface: &CutSurface<B>,
    w: &Rat<B>,
) -> Option<MuCut<B>> {
    let base = chart.pedal().add(&chart.normal().scale_rat(w)); // pedal + w·normal
    let u = chart.ruling();
    match surface {
        CutSurface::Plane { n, d } => {
            if dot3(n, n).sign() <= 0 {
                return None;
            }
            let nv = const_vec3(n);
            Some(MuCut {
                a: RatFunc::zero(),
                b: u.dot(&nv),
                c: base
                    .dot(&nv)
                    .sub(&RatFunc::from_poly(Poly::constant(d.clone()))),
            })
        }
        CutSurface::Cylinder {
            axis_point,
            axis_dir,
            r2,
        } => {
            let a2 = dot3(axis_dir, axis_dir);
            if a2.sign() <= 0 {
                return None;
            }
            let inv_a2 = a2.recip();
            let ax = const_vec3(axis_dir);
            let v0 = base.sub(&const_vec3(axis_point)); // pedal + w·n − p
            let ua = u.dot(&ax);
            let va = v0.dot(&ax);
            let a = u.dot(u).sub(&ua.mul(&ua).scale(&inv_a2));
            let b = v0
                .dot(u)
                .sub(&va.mul(&ua).scale(&inv_a2))
                .scale(&Rat::from_i128(2));
            let c = v0
                .dot(&v0)
                .sub(&va.mul(&va).scale(&inv_a2))
                .sub(&RatFunc::from_poly(Poly::constant(r2.clone())));
            Some(MuCut {
                a: a.reduce(),
                b: b.reduce(),
                c: c.reduce(),
            })
        }
        CutSurface::Quadric(q) => {
            let (m, b, c) = (&q.m, &q.b, &q.c);
            // `F(base + µ̂·u)` expanded in µ̂. `M` enters through `M + Mᵀ` in the cross term, which
            // is what makes an asymmetric `m` harmless.
            let mrow = |i: usize| const_vec3(&[m[i][0].clone(), m[i][1].clone(), m[i][2].clone()]);
            let quad = |p: &Vec3Rat<B>, q: &Vec3Rat<B>| {
                (0..3).fold(RatFunc::zero(), |acc, i| {
                    acc.add(&p.comp(i).mul(&q.dot(&mrow(i))))
                })
            };
            // `baseᵀMu + uᵀM base` — the two orderings differ unless `m` is symmetric.
            let cross = quad(&base, u).add(&quad(u, &base));
            let lin = |p: &Vec3Rat<B>| p.dot(&const_vec3(b));
            Some(MuCut {
                a: quad(u, u).reduce(),
                b: cross.add(&lin(u)).reduce(),
                c: quad(&base, &base)
                    .add(&lin(&base))
                    .add(&RatFunc::from_poly(Poly::constant(c.clone())))
                    .reduce(),
            })
        }
    }
}

/// The σ-sub-interval `[σ_lo + k·width, σ_lo + (k+1)·width]`.
fn subiv<B: Backend>(lo: &Rat<B>, width: &Rat<B>, k: usize) -> RatIv<B> {
    let a = lo.add(&width.mul(&Rat::from_i128(k as i128)));
    let b = a.add(width);
    RatIv::new(a, b)
}

/// The larger of two rationals.
fn max_rat<B: Backend>(a: Rat<B>, b: Rat<B>) -> Rat<B> {
    if a.cmp(&b) == core::cmp::Ordering::Less {
        b
    } else {
        a
    }
}

/// Certify the cut-fit obligation `sup_σ dist(C(σ, μ̂(σ)), {F=0}) ≤ ε` and gate it by
/// the DRC `ε < clearance/2`.
///
/// The checker **computes** the bound itself (its interval arithmetic is the trusted
/// part; it does not trust a searcher-supplied `ε`): it builds the rail point
/// `C(σ) = pedal + μ̂·ruling + w·normal` as a [`Vec3Rat`], subdivides `[σ_lo, σ_hi]`
/// into `subdiv` equal sub-intervals, and on each encloses the **geometric distance**
/// to the surface —
/// - **plane:** `|n·C − d| / |n|` (`|n|` a constant `√` enclosure);
/// - **cylinder:** `|√perp2(σ) − R|`, `perp2 = |C−p|² − ((C−p)·â)²/(â·â)`, `R = √r2`
///
/// — taking the maximum `ε`. Refining `subdiv` shrinks `ε`. Total:
/// `Verified(`[`ValidCutFit`]`)` when `ε < clearance/2`, `Unresolved(ε)` when not
/// (refine `subdiv`, or the oracle re-fits), or `Refuted(`[`CutFitFault`]`)` for a
/// degenerate span/surface or a pole in the evaluated residual.
pub fn cut_fit<B: Backend>(
    chart: &Chart<B>,
    cert: &CutFitCert<B>,
) -> Verdict<ValidCutFit<B>, CutFitFault, Rat<B>> {
    // The rail point on the cone: C(σ) = pedal + μ̂(σ)·ruling + w·normal, rational in σ.
    let c = chart
        .pedal()
        .add(&chart.ruling().scale(&cert.mu_hat))
        .add(&chart.normal().scale_rat(&cert.w));
    traced_cut_fit(
        &c,
        &cert.surface,
        &cert.span,
        cert.subdiv,
        &cert.clearance,
        &cert.cfg,
    )
}

/// Certify that a **p-curve** traces a cutting surface: the same obligation as [`cut_fit`], stated
/// over the curve's own parameter — `sup_t dist(X(t), {F=0}) ≤ ε` for the curve's 3-D image
/// `X(t)` ([`PCurve::lift`](crate::pcurve::PCurve::lift)) — and gated by the same DRC.
///
/// A graph rail is the special case `σ(t) = t`, so this subsumes [`cut_fit`] rather than competing
/// with it: both hand the same core the traced point as a rational vector function of whatever
/// parameter it is authored over. The generalization is what lets a cut **turn around in σ** — at
/// a solid cutter's tangent rulings, where a graph has to stop short — and so lets a closed cut be
/// certified as one curve instead of two branches plus bridges.
///
/// `Refuted(DegenerateSpan)` on an empty parameter span, `PoleInEval` where the traced point or
/// residual poles; a curve that drifts off the surface is `Unresolved(ε)`, never a wrong
/// `Verified`.
pub fn pcurve_cut_fit<B: Backend>(
    chart: &Chart<B>,
    curve: &crate::pcurve::PCurve<B>,
    surface: &CutSurface<B>,
    w: &Rat<B>,
    subdiv: usize,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Verdict<ValidCutFit<B>, CutFitFault, Rat<B>> {
    use core::cmp::Ordering;
    let (lo, hi) = (&curve.domain.lo, &curve.domain.hi);
    if lo.cmp(hi) != Ordering::Less {
        return Verdict::Refuted(CutFitFault::DegenerateSpan);
    }
    let n_sub = subdiv.max(1);
    let width = hi.sub(lo).div(&Rat::from_i128(n_sub as i128));
    let half = clearance.mul(&Rat::new(1, 2));
    let mut eps = Rat::from_i128(0);
    for k in 0..n_sub {
        crate::counters::bump_cut_eval();
        let t = subiv(lo, &width, k);
        // Enclose the curve in the DOMAIN, then evaluate the chart's own fields on the enclosed
        // σ. Composing the fields into `t` first would be the tidier expression but is numerically
        // ruinous: substituting an affine σ(t) into a degree-24 field denominator produces
        // monomial coefficients ~10²⁰⁰ whose true value is ~10², and the interval evaluation of
        // that cancellation straddles zero — a pole reported where the surface is perfectly
        // regular. Evaluating the fields at σ keeps every polynomial in its own well-scaled
        // variable; the price is the lost µ̂↔σ correlation across a piece, which shrinks with the
        // piece and is what `subdiv` refines.
        let [sig, mu] = match curve.eval_on(&t) {
            Some(b) => b,
            None => return Verdict::Refuted(CutFitFault::PoleInEval),
        };
        let x = match chart_point_on(chart, &sig, &mu, w) {
            Some(x) => x,
            None => return Verdict::Refuted(CutFitFault::PoleInEval),
        };
        let dist = match surface_distance_on(surface, &x, &half, cfg) {
            DistOn::Bound(d) | DistOn::Loose(d) => d,
            DistOn::Fault(f) => return Verdict::Refuted(f),
        };
        eps = max_rat(eps, dist);
    }
    if eps.cmp(&half) == Ordering::Less {
        Verdict::Verified(ValidCutFit {
            span: curve.domain.clone(),
            eps,
            clearance: clearance.clone(),
        })
    } else {
        Verdict::Unresolved(eps)
    }
}

/// A rational vector field enclosed over a parameter box. `None` where the shared denominator's
/// enclosure straddles zero — a possible pole, so the quotient is unbounded there.
fn vec3_on<B: Backend>(f: &Vec3Rat<B>, sig: &RatIv<B>) -> Option<[RatIv<B>; 3]> {
    let den = eval_poly_on(f.den(), sig);
    let inv = den
        .recip_pos()
        .or_else(|| den.neg().recip_pos().map(|r| r.neg()))?;
    Some([
        eval_poly_on(&f.num()[0], sig).mul(&inv),
        eval_poly_on(&f.num()[1], sig).mul(&inv),
        eval_poly_on(&f.num()[2], sig).mul(&inv),
    ])
}

/// The ruled-surface point `pedal(σ) + µ̂·ruling(σ) + w·normal(σ)` enclosed over σ- and µ̂-boxes.
fn chart_point_on<B: Backend>(
    chart: &Chart<B>,
    sig: &RatIv<B>,
    mu: &RatIv<B>,
    w: &Rat<B>,
) -> Option<[RatIv<B>; 3]> {
    let p = vec3_on(chart.pedal(), sig)?;
    let r = vec3_on(chart.ruling(), sig)?;
    let n = vec3_on(chart.normal(), sig)?;
    let wv = RatIv::point(w.clone());
    Some([
        p[0].add(&mu.mul(&r[0])).add(&wv.mul(&n[0])),
        p[1].add(&mu.mul(&r[1])).add(&wv.mul(&n[1])),
        p[2].add(&mu.mul(&r[2])).add(&wv.mul(&n[2])),
    ])
}

/// What bounding a box's distance to a surface produced.
enum DistOn<B: Backend> {
    /// A **certified** upper bound: every point of the box is within this of the surface.
    Bound(Rat<B>),
    /// No bound certified at this box, carrying a value that is guaranteed `≥ radius` — so a caller
    /// that folds it into `ε` and applies the `ε < radius` DRC gate can only reach `Unresolved`.
    Loose(Rat<B>),
    /// The surface data or the cut's placement is malformed — a refusal, not a refinement.
    Fault(CutFitFault),
}

/// An upper bound on the geometric distance from every point of a 3-D box to a cut surface.
///
/// `radius` is the working radius the caller will gate on (`clearance/2`). The two special surfaces
/// have closed-form distances and ignore it; [`CutSurface::Quadric`] has none, and uses it as the
/// radius of the ball its first-order bound is allowed to search — see [`quadric_distance_on`].
fn surface_distance_on<B: Backend>(
    surface: &CutSurface<B>,
    x: &[RatIv<B>; 3],
    radius: &Rat<B>,
    cfg: &DevConfig<B>,
) -> DistOn<B> {
    let closed_form = |d: Option<Rat<B>>| match d {
        Some(d) => DistOn::Bound(d),
        None => DistOn::Fault(CutFitFault::DegenerateSurface),
    };
    match surface {
        CutSurface::Quadric(q) => quadric_distance_on(&q.m, &q.b, &q.c, &q.nappe, x, radius, cfg),
        _ => closed_form(metric_distance_on(surface, x, cfg)),
    }
}

/// The closed-form distance bound for the two surfaces that have one. `None` on degenerate data.
fn metric_distance_on<B: Backend>(
    surface: &CutSurface<B>,
    x: &[RatIv<B>; 3],
    cfg: &DevConfig<B>,
) -> Option<Rat<B>> {
    match surface {
        CutSurface::Quadric(_) => None,
        CutSurface::Plane { n, d } => {
            let inv_norm = sqrt(&dot3(n, n), &cfg.sqrt_eps).recip_pos()?;
            let res = x[0]
                .mul(&RatIv::point(n[0].clone()))
                .add(&x[1].mul(&RatIv::point(n[1].clone())))
                .add(&x[2].mul(&RatIv::point(n[2].clone())))
                .sub(&RatIv::point(d.clone()));
            Some(abs_on(&res).mul(&inv_norm).hi().clone())
        }
        CutSurface::Cylinder {
            axis_point,
            axis_dir,
            r2,
        } => {
            let a2 = dot3(axis_dir, axis_dir);
            if a2.sign() <= 0 {
                return None;
            }
            let dv: Vec<RatIv<B>> = (0..3)
                .map(|i| x[i].sub(&RatIv::point(axis_point[i].clone())))
                .collect();
            let dot = |u: &[RatIv<B>], v: &[Rat<B>; 3]| {
                u[0].mul(&RatIv::point(v[0].clone()))
                    .add(&u[1].mul(&RatIv::point(v[1].clone())))
                    .add(&u[2].mul(&RatIv::point(v[2].clone())))
            };
            let d2 = dv[0]
                .mul(&dv[0])
                .add(&dv[1].mul(&dv[1]))
                .add(&dv[2].mul(&dv[2]));
            let ad = dot(&dv, axis_dir);
            let perp2 = d2.sub(&ad.mul(&ad).mul(&RatIv::point(a2.recip())));
            let rho = sqrt_on(&perp2, &cfg.sqrt_eps);
            Some(abs_on(&rho.sub(&sqrt(r2, &cfg.sqrt_eps))).hi().clone())
        }
    }
}

/// The **first-order distance bound** for a quadric, which has no closed-form distance.
///
/// ## The lemma
///
/// Let `F` be `C¹` on the closed ball `B̄(X, R)`, with `|∇F| ≥ g > 0` there. If `|F(X)| ≤ gR` then
/// `{F = 0}` meets the ball, within `|F(X)|/g` of `X`.
///
/// *Proof.* Take `F(X) > 0` (the other sign mirrors, `F(X) = 0` is trivial). Follow
/// `Y' = −∇F(Y)/|∇F(Y)|²`, so `F(Y(s)) = F(X) − s` falls at unit rate while the path advances at
/// speed `1/|∇F| ≤ 1/g`. For `s ≤ F(X) ≤ gR` the path has gone at most `s/g ≤ R`, so it is still in
/// the ball and the solution continues; at `s = F(X)` it stands on `{F = 0}`, at most `F(X)/g`
/// from `X`. ∎
///
/// ## Why the hypothesis is free, and why `R` is searched
///
/// The lemma needs `|F|/g ≤ R` — the bound must fit inside the ball it was derived in — and the
/// largest `R` worth trying is `clearance/2`, since a bound bigger than that fails the caller's DRC
/// gate anyway. So the hypothesis holds on precisely the runs that end in `Verified`: a bound too
/// weak to satisfy it is a bound too weak to pass the gate, and nothing is certified on a
/// hypothesis that failed.
///
/// Within that ceiling, `R` is **searched from small upward**, because `g` is a minimum over the
/// ball and a smaller ball has a larger one. A rail that really is on the surface has a tiny `|F|`
/// and succeeds at the smallest `R` on the first try; only a rail on its way to `Unresolved` pays
/// for the search. Reporting the largest `R` that happened to work would be sound but would inflate
/// ε by the ratio of the clearance to the true error, which is the whole quantity being measured.
///
/// The bound is *self-validating*: it never has to be told that `M` really describes a cone. A
/// quadric with no real points, or none nearby, cannot pass, because the hypotheses it would need
/// are the ones that fail.
///
/// ## What it will not certify
///
/// The ball has to avoid the surface's **singular locus** — a cone's apex, a cylinder's axis —
/// because `∇F` dies there. Since the ball must be at least as big as the bound it carries, this
/// bites when a cut's error is an appreciable fraction of the feature's own radius: measured on the
/// device's `R = 1/5` drill, an error of `5·10⁻⁴` certifies at a factor `1.4` off the exact
/// distance, while an error of `6·10⁻²` — a chord across a third of the hole — cannot be bounded at
/// all and reads `Unresolved`. That is the honest verdict rather than a defect: at that scale the
/// distance to the surface is no longer a first-order quantity, and the cut wants re-authoring, not
/// a looser certificate.
///
/// ## The nappe
///
/// The zero the lemma finds lies in the ball, so bounding the distance to `{F = 0}` bounds the
/// distance to the **authored nappe** only if the whole ball is on that nappe's side. That is one
/// extra enclosure — the selector over the same inflated box — and it discharges both
/// `docs/cutter-extrude-design.md` §4.1 conditions at once: a band reaching the mirror nappe fails
/// it, and so does a band reaching the apex, which sits on the selector's own boundary plane.
fn quadric_distance_on<B: Backend>(
    m: &[[Rat<B>; 3]; 3],
    b: &[Rat<B>; 3],
    c: &Rat<B>,
    nappe: &Nappe<B>,
    x: &[RatIv<B>; 3],
    radius: &Rat<B>,
    cfg: &DevConfig<B>,
) -> DistOn<B> {
    let dot_iv = |v: &[Rat<B>; 3], y: &[RatIv<B>; 3]| {
        y[0].mul(&RatIv::point(v[0].clone()))
            .add(&y[1].mul(&RatIv::point(v[1].clone())))
            .add(&y[2].mul(&RatIv::point(v[2].clone())))
    };
    let inflate = |r: &Rat<B>| -> [RatIv<B>; 3] {
        core::array::from_fn(|i| RatIv::new(x[i].lo().sub(r), x[i].hi().add(r)))
    };

    // The nappe condition is a statement about the *authored geometry*, not about this bound's
    // working precision, so it is checked at the full working radius however small a ball the
    // bound below ends up needing: a cut is required to clear the apex by `clearance/2`, since
    // nearer than that "inside" is not a settled question.
    if dot_iv(&nappe.n, &inflate(radius))
        .sub(&RatIv::point(nappe.d.clone()))
        .lo()
        .sign()
        <= 0
    {
        return DistOn::Fault(CutFitFault::NappeCrossed);
    }

    // `|F|` over the box itself — the lemma is applied at each of its points, so this is the only
    // quantity that does not depend on the ball.
    let mut f = RatIv::point(c.clone());
    for i in 0..3 {
        let row: [Rat<B>; 3] = core::array::from_fn(|j| m[i][j].clone());
        f = f
            .add(&x[i].mul(&dot_iv(&row, x)))
            .add(&x[i].mul(&RatIv::point(b[i].clone())));
    }
    let f_hi = abs_on(&f).hi().clone();

    // Grow the ball from small to the full radius and take the **first** one that contains its own
    // bound. Both the tightness and the cost live here: `g` is a minimum over the ball, so a
    // smaller ball gives a larger `g` and a tighter `ε` — and a rail that really is on the surface
    // has a tiny `|F|`, so it succeeds on the first and smallest try. Iterating only happens on the
    // way to `Unresolved`.
    const STEPS: u32 = 4;
    let mut widest = None;
    for k in (0..=STEPS).rev() {
        let r = radius.mul(&Rat::new(1, 1i128 << k));
        let ball = inflate(&r);
        // `g`: a lower bound on `|∇F| = |(M + Mᵀ)Y + b|` over the ball. A component whose enclosure
        // straddles zero contributes nothing, which is the sound reading.
        let mut g2 = Rat::from_i128(0);
        for i in 0..3 {
            let row: [Rat<B>; 3] = core::array::from_fn(|j| m[i][j].add(&m[j][i]));
            let gi = abs_on(&dot_iv(&row, &ball).add(&RatIv::point(b[i].clone())));
            g2 = g2.add(&gi.lo().mul(gi.lo()));
        }
        let g = sqrt(&g2, &cfg.sqrt_eps).lo().clone();
        if g.sign() <= 0 {
            // This ball reaches the quadric's own vertex, where the gradient dies and no
            // first-order bound exists. A smaller ball may still clear it, so try the next one.
            continue;
        }
        // Rounded **up** to the standard fixed-precision budget: the quotient of two
        // deep-in-the-chart rationals otherwise carries hundreds of digits into an `ε` whose only
        // uses are `max` and a comparison. Rounding outward keeps it an upper bound.
        let e = RatIv::point(f_hi.div(&g))
            .round_out(ROUND_BITS)
            .hi()
            .clone();
        if e.cmp(&r) != core::cmp::Ordering::Greater {
            return DistOn::Bound(e);
        }
        widest = Some(e);
    }
    // Nothing certified. Report at least `radius`, so a caller folding this into `ε` and applying
    // the `ε < radius` DRC gate cannot mistake it for a bound.
    DistOn::Loose(max_rat(
        widest.unwrap_or_else(|| radius.clone()),
        radius.clone(),
    ))
}

/// A polynomial enclosed over an interval (interval Horner).
fn eval_poly_on<B: Backend>(p: &Poly<B>, x: &RatIv<B>) -> RatIv<B> {
    let mut acc = RatIv::point(Rat::from_i128(0));
    for c in p.coeffs().iter().rev() {
        acc = acc.mul(x).add(&RatIv::point(c.clone()));
    }
    acc
}

/// The shared core: the `traced` point as a rational vector function of its own parameter, the
/// rigorous `sup` of its distance to `surface` over `span`, and the DRC gate. Both the graph and
/// p-curve entry points differ only in how `traced` is built.
fn traced_cut_fit<B: Backend>(
    traced: &Vec3Rat<B>,
    surface: &CutSurface<B>,
    span: &Interval<B>,
    subdiv: usize,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Verdict<ValidCutFit<B>, CutFitFault, Rat<B>> {
    use core::cmp::Ordering;
    let (lo, hi) = (&span.lo, &span.hi);
    if lo.cmp(hi) != Ordering::Less {
        return Verdict::Refuted(CutFitFault::DegenerateSpan);
    }
    let n_sub = subdiv.max(1);
    let width = hi.sub(lo).div(&Rat::from_i128(n_sub as i128));
    let half = clearance.mul(&Rat::new(1, 2));

    let eps = match surface {
        // No closed-form distance, so this arm cannot enclose a symbolic residual the way the two
        // below do: the first-order bound works on a 3-D ball around the traced point, so the point
        // itself has to be enclosed first. The price is the lost cancellation — a symbolic residual
        // benefits from the surface equation collapsing against the chart fields, and a box does
        // not — which shows up as needing more `subdiv` for the same ε, not as a weaker claim.
        CutSurface::Quadric(q) => {
            let mut eps = Rat::from_i128(0);
            for k in 0..n_sub {
                let sig = subiv(lo, &width, k);
                let x = match vec3_on(traced, &sig) {
                    Some(x) => x,
                    None => return Verdict::Refuted(CutFitFault::PoleInEval),
                };
                match quadric_distance_on(&q.m, &q.b, &q.c, &q.nappe, &x, &half, cfg) {
                    DistOn::Bound(d) | DistOn::Loose(d) => eps = max_rat(eps, d),
                    DistOn::Fault(f) => return Verdict::Refuted(f),
                }
            }
            eps
        }
        CutSurface::Plane { n, d } => {
            let norm = sqrt(&dot3(n, n), &cfg.sqrt_eps); // |n| enclosure
            let inv_norm = match norm.recip_pos() {
                Some(iv) => iv,
                None => return Verdict::Refuted(CutFitFault::DegenerateSurface),
            };
            // residual(σ) = n·C(σ) − d, a rational function of σ.
            let residual = traced
                .dot(&const_vec3(n))
                .sub(&RatFunc::from_poly(Poly::constant(d.clone())))
                .reduce();
            let mut eps = Rat::from_i128(0);
            for k in 0..n_sub {
                let sig = subiv(lo, &width, k);
                let res = match eval_ratfunc_on(&residual, &sig) {
                    Some(r) => r,
                    None => return Verdict::Refuted(CutFitFault::PoleInEval),
                };
                // distance = |residual| / |n|
                let dist = abs_on(&res).mul(&inv_norm);
                eps = max_rat(eps, dist.hi().clone());
            }
            eps
        }
        CutSurface::Cylinder {
            axis_point,
            axis_dir,
            r2,
        } => {
            let a2 = dot3(axis_dir, axis_dir); // â·â
            if a2.sign() <= 0 {
                return Verdict::Refuted(CutFitFault::DegenerateSurface);
            }
            let inv_a2 = a2.recip();
            let dvec = traced.sub(&const_vec3(axis_point)); // X − p
            let ax = const_vec3(axis_dir);
            let axdot = dvec.dot(&ax); // (X − p)·â
            // perp2(σ) = |X−p|² − ((X−p)·â)² / (â·â), the squared distance to the axis.
            let perp2 = dvec
                .dot(&dvec)
                .sub(&axdot.mul(&axdot).scale(&inv_a2))
                .reduce();
            let r = sqrt(r2, &cfg.sqrt_eps); // R = √r2 enclosure
            let mut eps = Rat::from_i128(0);
            for k in 0..n_sub {
                let sig = subiv(lo, &width, k);
                let p2 = match eval_ratfunc_on(&perp2, &sig) {
                    Some(p) => p,
                    None => return Verdict::Refuted(CutFitFault::PoleInEval),
                };
                let rho = sqrt_on(&p2, &cfg.sqrt_eps); // √perp2 = distance to axis
                let dist = abs_on(&rho.sub(&r)); // |ρ − R|
                eps = max_rat(eps, dist.hi().clone());
            }
            eps
        }
    };

    if eps.cmp(&half) == Ordering::Less {
        Verdict::Verified(ValidCutFit {
            span: span.clone(),
            eps,
            clearance: clearance.clone(),
        })
    } else {
        Verdict::Unresolved(eps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::devices::cone;

    type Q = Rat<Bignum>;

    fn to_f64(r: &Q) -> f64 {
        let (n, d) = r.numer_denom_decimal();
        n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
    }
    fn ivl(lo: i128, hi: i128) -> Interval<Bignum> {
        Interval {
            lo: Q::from_i128(lo),
            hi: Q::from_i128(hi),
        }
    }

    /// A **graph** p-curve certifies the same exact rail the graph checker does — with the
    /// tightness this path can offer. Because it encloses in the domain rather than composing
    /// (see [`pcurve_cut_fit`]), it forgoes the symbolic cancellation that lets [`cut_fit`] report
    /// ε ≈ 0 on an exact rail, and its bound is **first-order** in `subdiv` (measured: 8× the
    /// subdivisions buys ~8× the tightness). Graph rails therefore keep using `cut_fit`; this path
    /// exists for the curves `cut_fit` cannot express at all.
    #[test]
    fn a_graph_pcurve_certifies_the_same_exact_rail() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        let d = Q::from_i128(1);
        let surface = CutSurface::Plane {
            n: n.clone(),
            d: d.clone(),
        };
        let rail = plane_cut_rail(&chart, &n, &d);
        let curve = crate::pcurve::PCurve::graph(
            rail,
            Interval {
                lo: Q::from_i128(1),
                hi: Q::new(5, 4),
            },
        );
        match pcurve_cut_fit(
            &chart,
            &curve,
            &surface,
            &Q::from_i128(0),
            512,
            &Q::from_i128(1),
            &DevConfig::tight(),
        ) {
            Verdict::Verified(v) => assert!(
                v.eps < Q::new(1, 100),
                "the enclosure must be tight at this subdivision, got {}",
                to_f64(&v.eps)
            ),
            other => panic!("expected Verified, got {:?}", verdict_tag(&other)),
        }
    }

    /// **The capability the graph model cannot express**: a cut curve that *turns around in σ*
    /// still certifies. The same exact plane rail is re-parametrized by `σ(t) = 1 − t²/4`, which
    /// reverses at `t = 0` — so `dµ̂/dσ` is unbounded there and no `µ̂ = f(σ)` covers the curve in
    /// one piece — yet the traced point lies on the plane throughout and the certificate says so
    /// at ε ≈ 0. The obligation is stated over the curve's own parameter, so it never sees the
    /// turn as a singularity.
    #[test]
    fn a_curve_that_turns_around_in_sigma_still_certifies() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        let d = Q::from_i128(1);
        let surface = CutSurface::Plane {
            n: n.clone(),
            d: d.clone(),
        };
        let rail = plane_cut_rail(&chart, &n, &d);
        // σ(t) = 1 − t²/4 on t ∈ [−1, 1]: σ ∈ [3/4, 1], reversing at t = 0.
        let sigma = RatFunc::from_poly(Poly::from_coeffs(vec![
            Q::from_i128(1),
            Q::from_i128(0),
            Q::new(-1, 4),
        ]));
        let mu = crate::pcurve::compose(&rail, &sigma).expect("composable");
        let curve = crate::pcurve::PCurve {
            sigma,
            mu,
            domain: ivl(-1, 1),
        };
        assert_eq!(
            curve.sigma_turning_points(64, 40).unwrap().len(),
            1,
            "the fixture must genuinely turn around in σ"
        );
        match pcurve_cut_fit(
            &chart,
            &curve,
            &surface,
            &Q::from_i128(0),
            512,
            &Q::from_i128(1),
            &DevConfig::tight(),
        ) {
            Verdict::Verified(v) => assert!(
                v.eps < Q::new(1, 20),
                "a turning curve on the plane certifies, got {}",
                to_f64(&v.eps)
            ),
            other => panic!("expected Verified, got {:?}", verdict_tag(&other)),
        }
    }

    /// Fail-closed: nudge the traced curve off the cutting surface and the certificate refuses to
    /// call it a cut — a loose curve is `Unresolved`, never a wrong `Verified`.
    #[test]
    fn a_curve_off_the_surface_is_not_certified() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        let d = Q::from_i128(1);
        let surface = CutSurface::Plane {
            n: n.clone(),
            d: d.clone(),
        };
        let drifted =
            plane_cut_rail(&chart, &n, &d).add(&RatFunc::from_poly(Poly::constant(Q::new(1, 10))));
        let curve = crate::pcurve::PCurve::graph(
            drifted,
            Interval {
                lo: Q::from_i128(1),
                hi: Q::new(5, 4),
            },
        );
        match pcurve_cut_fit(
            &chart,
            &curve,
            &surface,
            &Q::from_i128(0),
            512,
            &Q::new(1, 100),
            &DevConfig::tight(),
        ) {
            Verdict::Unresolved(e) => assert!(
                e > Q::new(1, 1000),
                "the drift must show up in ε, got {}",
                to_f64(&e)
            ),
            other => panic!("expected Unresolved, got {:?}", verdict_tag(&other)),
        }
    }

    /// **The hole's shape, on the real device drill.** The graph model bridges the two tangent
    /// rulings with straight chords whose length has a floor: over every margin × degree × subdiv
    /// rung it never gets below ~30% of the hole's height (the shipped rung gives 48%). The
    /// p-curve loop walks the branches to their meeting points instead, so the residual gap is set
    /// by the bisected root's resolution — here below a **thousandth of a percent** of the hole,
    /// five orders of magnitude better, and it is *inside* the reported ε rather than unaccounted.
    #[test]
    fn the_quadric_cut_loop_closes_at_its_tangent_rulings() {
        let chart = fixtures::devices::cone_wrap();
        let surface = CutSurface::Cylinder {
            axis_point: [Q::new(-1, 2), Q::new(27, 10), Q::from_i128(0)],
            axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
            r2: Q::new(1, 40),
        };
        let cfg = DevConfig::tight();
        // The head window, between the drill's first two tangent rulings.
        let roots = crate::pcurve::scan_roots(
            &cut_mu_form(&chart, &surface, &Q::from_i128(0))
                .unwrap()
                .disc(),
            &Q::new(-5, 4),
            &Q::new(5, 4),
            512,
            60,
        )
        .expect("disc roots");
        let window = Interval {
            lo: roots[0].clone(),
            hi: roots[1].clone(),
        };
        let loop_ = match quadric_cut_loop(
            &chart,
            &surface,
            &window,
            &Q::from_i128(0),
            12,
            &Q::from_i128(1),
            &cfg,
        ) {
            Verdict::Verified(l) => l,
            other => panic!("the cut loop must certify, got {:?}", verdict_tag(&other)),
        };
        // A closed loop through both tangents: two branches plus the two shared extreme vertices.
        assert!(loop_.pieces.len() >= 8, "a closed loop of pieces");
        // The hole's µ̂-height, for scale.
        let mc = cut_mu_form(&chart, &surface, &Q::from_i128(0)).unwrap();
        let smid = window.lo.add(&window.hi).mul(&Q::new(1, 2));
        let (_, h_mid) = mc.branch_at(&smid, &cfg.sqrt_eps).unwrap();
        let height = h_mid.mul(&Q::from_i128(2));
        assert!(
            loop_.tangent_gap.mul(&Q::from_i128(1_000)) < height,
            "the tangent gap must be far below the graph model's ~30% floor: gap {} vs height {}",
            to_f64(&loop_.tangent_gap),
            to_f64(&height)
        );
        // And the loop really tracks the drill: its certified distance to the cylinder beats the
        // graph model on this very window, where the *best* rung over the whole margin × degree ×
        // subdiv ladder was ε ≈ 0.257 and the rung that ships gives 0.203 — with a straight 30–48%
        // chord across the tangents on top. Note the bound here is first-order in the per-piece
        // subdivision (the box form of `pcurve_cut_fit`), so it reads looser than the loop's true
        // deviation; `segments` and that subdivision are the handles.
        assert!(
            loop_.eps < Q::new(1, 10),
            "loop ε must beat the graph model's 0.257, got {}",
            to_f64(&loop_.eps)
        );
    }

    // — AUTH.1e.4: the multi-wall band loop. —

    /// A closed rectilinear profile in the frame plane, as `arrange2d` segment edges.
    fn poly_edges(pts: &[(Q, Q)], src: u32) -> Vec<geom::content::Edge<Bignum>> {
        let n = pts.len();
        (0..n)
            .map(|i| {
                let ((sx, sy), (ex, ey)) = (&pts[i], &pts[(i + 1) % n]);
                let (a, b) = (sy.sub(ey), ex.sub(sx));
                let c = a.mul(sx).add(&b.mul(sy)).neg();
                geom::content::Edge::Seg(Box::new(geom::content::SegPiece {
                    line: geom::content::Line { a, b, c },
                    start: geom::content::Point2::from_rat(sx.clone(), sy.clone()),
                    end: geom::content::Point2::from_rat(ex.clone(), ey.clone()),
                    orient: geom::content::Orient::Ccw,
                    source: geom::content::CurveId(src),
                }))
            })
            .collect()
    }

    /// A whole circle as its two x-monotone halves — `arrange2d`'s canonical decomposition. `r` is
    /// rational so the halves can name their extreme points exactly.
    fn circle_edges(cx: &Q, cy: &Q, r: &Q, src: u32) -> Vec<geom::content::Edge<Bignum>> {
        use geom::content::{ArcPiece, Circle, CurveId, Edge, Half, Orient, Point2, Winding};
        let (lo, hi) = (cx.sub(r), cx.add(r));
        let circle = Circle {
            cx: cx.clone(),
            cy: cy.clone(),
            r2: r.mul(r),
        };
        [Half::Upper, Half::Lower]
            .into_iter()
            .map(|half| {
                Edge::Arc(Box::new(ArcPiece {
                    circle: circle.clone(),
                    half,
                    x_lo: lattice::Surd::from_rat(lo.clone()),
                    x_hi: lattice::Surd::from_rat(hi.clone()),
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

    /// The frame the test profiles are drawn in: the physical xy-plane.
    fn xy_frame() -> crate::extrude::Frame<Bignum> {
        crate::extrude::Frame::new(
            [Q::from_i128(0), Q::from_i128(0), Q::from_i128(0)],
            [Q::from_i128(1), Q::from_i128(0), Q::from_i128(0)],
            [Q::from_i128(0), Q::from_i128(1), Q::from_i128(0)],
        )
        .expect("independent axes")
    }

    /// The σ-window station targeting would hand a multi-walled cutter: the first two tangent
    /// rulings of its **bounding circle**'s wall (an all-affine profile has no window of its own —
    /// `docs/cutter-extrude-design.md` §6).
    fn bounding_window(
        chart: &geom::chart::Chart<Bignum>,
        bound: &CutSurface<Bignum>,
    ) -> Interval<Bignum> {
        let roots = crate::pcurve::scan_roots(
            &cut_mu_form(chart, bound, &Q::from_i128(0)).unwrap().disc(),
            &Q::new(-5, 4),
            &Q::new(5, 4),
            512,
            60,
        )
        .expect("the bounding circle's tangent rulings");
        assert!(roots.len() >= 2, "a bounding window needs two tangents");
        Interval {
            lo: roots[0].clone(),
            hi: roots[1].clone(),
        }
    }

    /// Every µ̂ the emitted loop takes at the ruling `s`, by evaluating each straight piece whose
    /// σ-band contains it. This reads the **emitted geometry**, not the builder's own bookkeeping.
    fn loop_mu_at(pieces: &[crate::pcurve::PCurve<Bignum>], s: &Q) -> Vec<Q> {
        use core::cmp::Ordering::{Equal, Less};
        let mut out = Vec::new();
        for p in pieces {
            let a = p
                .sigma
                .eval(&p.domain.lo)
                .expect("a straight piece evaluates");
            let b = p
                .sigma
                .eval(&p.domain.hi)
                .expect("a straight piece evaluates");
            if a.cmp(&b) == Equal {
                continue;
            }
            let (lo, hi) = if a.cmp(&b) == Less {
                (&a, &b)
            } else {
                (&b, &a)
            };
            if s.cmp(lo) == Less || hi.cmp(s) == Less {
                continue;
            }
            let f = s.sub(&a).div(&b.sub(&a));
            let t = p.domain.lo.add(&p.domain.hi.sub(&p.domain.lo).mul(&f));
            out.push(p.mu.eval(&t).expect("a straight piece evaluates"));
        }
        out
    }

    /// **The multi-wall loop is the right size, checked against geometry it does not share.** A
    /// square prism's hole must be wider than the hole of the cylinder inscribed in it and narrower
    /// than the one circumscribing it — `disc(h) ⊂ square(h) ⊂ disc(h√2)`, so the same inclusion
    /// holds for every ruling's chord. Both bounds come from the **metric** cylinder path
    /// (`MuCut::branch_at` on `CutSurface::Cylinder`), which shares no code with the wall-crossing
    /// band builder, so this is a differential and not a restatement.
    ///
    /// The window handed in is the *bounding circle's* — a strict superset, as station targeting
    /// supplies for an all-affine profile — so the loop also has to find its own σ-extent.
    ///
    /// Measured across the window: band `0.2364 / 0.2342 / 0.2323` against inner `0.1615 / 0.2303 /
    /// 0.1639` and outer `0.2841 / 0.3257 / 0.2800` — a real squeeze at the middle ruling (1.7%
    /// above the inscribed chord), and a band that is plainly neither disc.
    #[test]
    fn a_square_prism_cuts_a_band_between_its_inscribed_and_circumscribed_discs() {
        use core::cmp::Ordering::Greater;
        let chart = fixtures::devices::cone_wrap();
        let cfg = DevConfig::tight();
        let (cx, cy, h) = (Q::new(-1, 2), Q::new(27, 10), Q::new(1, 8));
        let corners: Vec<(Q, Q)> = [(-1, -1), (1, -1), (1, 1), (-1, 1)]
            .into_iter()
            .map(|(i, j)| {
                (
                    cx.add(&h.mul(&Q::from_i128(i))),
                    cy.add(&h.mul(&Q::from_i128(j))),
                )
            })
            .collect();
        let profile = poly_edges(&corners, 0);
        let cast = crate::extrude::Cast::new(
            xy_frame(),
            crate::extrude::Apex::direction([Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)])
                .expect("a real direction"),
        )
        .expect("the apex is off the frame plane");
        let walls = cast.carrier_walls(&profile).expect("four distinct lines");
        assert_eq!(walls.len(), 4, "a square has four carriers");

        // The two comparison cylinders, built by the metric path alone.
        let cyl = |r2: Q| CutSurface::Cylinder {
            axis_point: [cx.clone(), cy.clone(), Q::from_i128(0)],
            axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
            r2,
        };
        let inscribed = cyl(h.mul(&h));
        let circumscribed = cyl(h.mul(&h).mul(&Q::from_i128(2)));

        let window = bounding_window(&chart, &circumscribed);
        let zero = Q::from_i128(0);
        let inside = |s: &Q, mu: &Q| -> Option<bool> {
            let p = chart.surface(mu, &zero).eval(s)?;
            cast.contains(&p, &profile)
        };
        let loop_ = match shadow_cut_loop(
            &chart,
            &walls,
            inside,
            &window,
            &zero,
            16,
            &Q::from_i128(1),
            &cfg,
        ) {
            Verdict::Verified(l) => l,
            other => panic!(
                "the square's loop must certify, got {:?}",
                verdict_tag(&other)
            ),
        };
        assert!(loop_.pieces.len() >= 8, "a closed band of pieces");

        // At three rulings across the window, the emitted band sits between the two discs' chords.
        let mut probed = 0;
        for k in 1..4 {
            let s = window.lo.add(&window.hi.sub(&window.lo).mul(&Q::new(k, 4)));
            let mus = loop_mu_at(&loop_.pieces, &s);
            let (Some(lo), Some(hi)) = (
                mus.iter().min_by(|a, b| a.cmp(b)),
                mus.iter().max_by(|a, b| a.cmp(b)),
            ) else {
                continue; // outside the footprint's own extent
            };
            let band = hi.sub(lo);
            let chord = |surface: &CutSurface<Bignum>| -> Option<Q> {
                let (_, hh) = cut_mu_form(&chart, surface, &zero)?.branch_at(&s, &cfg.sqrt_eps)?;
                Some(hh.mul(&Q::from_i128(2)))
            };
            let (Some(inner), Some(outer)) = (chord(&inscribed), chord(&circumscribed)) else {
                continue;
            };
            probed += 1;
            let slack = Q::new(1, 1_000_000);
            assert!(
                band.add(&slack).cmp(&inner) == Greater,
                "the square's band must contain the inscribed disc's: {} vs {}",
                to_f64(&band),
                to_f64(&inner)
            );
            assert!(
                outer.add(&slack).cmp(&band) == Greater,
                "and stay inside the circumscribed disc's: {} vs {}",
                to_f64(&band),
                to_f64(&outer)
            );
        }
        assert!(probed >= 2, "at least two rulings must be comparable");

        // And the band really is bounded by several walls: each emitted vertex is on one of the
        // four planes, and between them the vertices use at least three. A loop that had silently
        // followed one wall — the AUTH.1e.2 hardcode's failure mode — could not.
        let mut used: Vec<usize> = Vec::new();
        for p in &loop_.pieces {
            let [s, mu] = p.eval(&p.domain.lo).expect("a straight piece evaluates");
            let x = chart
                .surface(&mu, &zero)
                .eval(&s)
                .expect("a regular ruling");
            let mut best: Option<(usize, Q)> = None;
            for (wi, wall) in walls.iter().enumerate() {
                let CutSurface::Plane { n, d } = wall else {
                    panic!("a straight profile edge sweeps a plane")
                };
                let r = dot3(n, &x).sub(d);
                // Distance², so no root is taken: `(n·X − d)² / |n|²`.
                let dist2 = r.mul(&r).div(&dot3(n, n));
                if best.as_ref().is_none_or(|(_, b)| b.cmp(&dist2) == Greater) {
                    best = Some((wi, dist2));
                }
            }
            let (wi, dist2) = best.expect("four walls");
            assert!(
                dist2.cmp(&Q::new(1, 1_000_000)) != Greater,
                "every loop vertex must sit on one of the walls, got dist² {}",
                to_f64(&dist2)
            );
            if !used.contains(&wi) {
                used.push(wi);
            }
        }
        assert!(
            used.len() >= 3,
            "a square hole's boundary must run on several walls, used {}",
            used.len()
        );
    }

    /// **The ring is refused, deliberately.** A profile with a hole of its own shadows every ruling
    /// in *two* stretches, which a band cannot express — so the builder says so with a typed fault
    /// instead of picking a stretch and shipping a hole that is not the one drawn. Before this it
    /// failed closed only by accident, on the window search declining a shape it could not read.
    #[test]
    fn a_ring_profile_is_refused_rather_than_approximated() {
        let chart = fixtures::devices::cone_wrap();
        let (cx, cy) = (Q::new(-1, 2), Q::new(27, 10));
        let mut profile = circle_edges(&cx, &cy, &Q::new(1, 4), 0);
        profile.extend(circle_edges(&cx, &cy, &Q::new(1, 8), 1));
        let cast = crate::extrude::Cast::new(
            xy_frame(),
            crate::extrude::Apex::direction([Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)])
                .expect("a real direction"),
        )
        .expect("the apex is off the frame plane");
        let walls = cast.carrier_walls(&profile).expect("two distinct circles");
        assert_eq!(walls.len(), 2, "a ring has two carriers");
        let window = bounding_window(&chart, &walls[0]);
        let zero = Q::from_i128(0);
        let inside = |s: &Q, mu: &Q| -> Option<bool> {
            let p = chart.surface(mu, &zero).eval(s)?;
            cast.contains(&p, &profile)
        };
        match shadow_cut_loop(
            &chart,
            &walls,
            inside,
            &window,
            &zero,
            16,
            &Q::from_i128(1),
            &DevConfig::tight(),
        ) {
            Verdict::Refuted(CutFitFault::ShadowNotSimple) => {}
            other => panic!(
                "a ring must be refused as not-a-band, got {:?}",
                verdict_tag(&other)
            ),
        }
    }

    /// The exact offset-plane rail verifies with ε ≈ 0 (the residual is identically 0).
    #[test]
    fn plane_cut_rail_verifies_near_zero() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)]; // z = d plane
        let d = Q::from_i128(1);
        let mu_hat = plane_cut_rail(&chart, &n, &d);
        let cert = CutFitCert {
            mu_hat,
            w: Q::from_i128(0),
            surface: CutSurface::Plane { n, d },
            span: ivl(1, 3),
            subdiv: 8,
            clearance: Q::new(1, 100),
            cfg: DevConfig::tight(),
        };
        match cut_fit(&chart, &cert) {
            Verdict::Verified(v) => assert!(
                v.eps <= Q::new(1, 1_000_000),
                "exact rail ε ≈ 0, got {}",
                to_f64(&v.eps)
            ),
            other => panic!("expected Verified, got {:?}", verdict_tag(&other)),
        }
    }

    /// An offset rail (μ̂ + δ) is off the plane: Unresolved at a tight clearance,
    /// Verified at a generous one; ε shrinks as the σ-subdivision refines.
    #[test]
    fn offset_plane_rail_is_unresolved_then_verified() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        let d = Q::from_i128(1);
        let delta = Q::new(1, 20); // push the rail off the plane
        let mu_hat = plane_cut_rail(&chart, &n, &d).add(&RatFunc::from_poly(Poly::constant(delta)));
        let mk = |clearance: Q, subdiv: usize| CutFitCert {
            mu_hat: mu_hat.clone(),
            w: Q::from_i128(0),
            surface: CutSurface::Plane {
                n: n.clone(),
                d: d.clone(),
            },
            span: ivl(1, 3),
            subdiv,
            clearance,
            cfg: DevConfig::tight(),
        };
        // Tight clearance ⇒ Unresolved with a positive ε.
        let eps_coarse = match cut_fit(&chart, &mk(Q::new(1, 1000), 4)) {
            Verdict::Unresolved(e) => e,
            other => panic!("expected Unresolved, got {:?}", verdict_tag(&other)),
        };
        assert!(eps_coarse.sign() > 0, "offset ⇒ positive ε");
        // Refining shrinks (or holds) the certified sup.
        let eps_fine = match cut_fit(&chart, &mk(Q::new(1, 1000), 32)) {
            Verdict::Unresolved(e) => e,
            Verdict::Verified(v) => v.eps,
            other => panic!("unexpected {:?}", verdict_tag(&other)),
        };
        assert!(eps_fine <= eps_coarse, "ε tightens with subdiv");
        // Generous clearance ⇒ Verified.
        match cut_fit(&chart, &mk(Q::from_i128(10), 16)) {
            Verdict::Verified(_) => {}
            other => panic!("expected Verified, got {:?}", verdict_tag(&other)),
        }
    }

    /// A degenerate span and a degenerate surface are Refuted (structural, not loose).
    #[test]
    fn degenerate_span_and_surface_are_refuted() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        let base = |span: Interval<Bignum>, surface: CutSurface<Bignum>| CutFitCert {
            mu_hat: plane_cut_rail(&chart, &n, &Q::from_i128(1)),
            w: Q::from_i128(0),
            surface,
            span,
            subdiv: 4,
            clearance: Q::from_i128(1),
            cfg: DevConfig::tight(),
        };
        // σ_lo ≥ σ_hi.
        match cut_fit(
            &chart,
            &base(
                ivl(2, 1),
                CutSurface::Plane {
                    n: n.clone(),
                    d: Q::from_i128(1),
                },
            ),
        ) {
            Verdict::Refuted(CutFitFault::DegenerateSpan) => {}
            other => panic!("expected DegenerateSpan, got {:?}", verdict_tag(&other)),
        }
        // Zero plane normal.
        match cut_fit(
            &chart,
            &base(
                ivl(1, 2),
                CutSurface::Plane {
                    n: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(0)],
                    d: Q::from_i128(1),
                },
            ),
        ) {
            Verdict::Refuted(CutFitFault::DegenerateSurface) => {}
            other => panic!("expected DegenerateSurface, got {:?}", verdict_tag(&other)),
        }
    }

    /// A rail with a pole inside the span is Refuted (PoleInEval), not silently wrong.
    #[test]
    fn a_pole_in_span_is_refused() {
        let chart = cone();
        let n = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        // μ̂ = 1/(σ − 2), a pole at σ = 2 ∈ [1, 3].
        let mu_hat = RatFunc::new(
            Poly::constant(Q::from_i128(1)),
            Poly::from_coeffs(vec![Q::from_i128(-2), Q::from_i128(1)]),
        );
        let cert = CutFitCert {
            mu_hat,
            w: Q::from_i128(0),
            surface: CutSurface::Plane {
                n,
                d: Q::from_i128(1),
            },
            span: ivl(1, 3),
            subdiv: 4,
            clearance: Q::from_i128(1),
            cfg: DevConfig::tight(),
        };
        match cut_fit(&chart, &cert) {
            Verdict::Refuted(CutFitFault::PoleInEval) => {}
            other => panic!("expected PoleInEval, got {:?}", verdict_tag(&other)),
        }
    }

    /// The plane µ̂-form vanishes identically on the exact plane rail: `b·µ̂₁ + c ≡ 0`.
    #[test]
    fn the_plane_mu_form_vanishes_on_the_exact_rail() {
        let chart = cone();
        let n = [Q::from_i128(1), Q::new(-1, 2), Q::from_i128(2)]; // a generic plane
        let d = Q::new(3, 4);
        let rail = plane_cut_rail(&chart, &n, &d);
        let form = cut_mu_form(&chart, &CutSurface::Plane { n, d }, &Q::from_i128(0)).unwrap();
        assert!(form.a.is_zero(), "a plane is affine in µ̂");
        let residual = form.b.mul(&rail).add(&form.c).reduce();
        assert!(residual.is_zero(), "s(σ, µ̂₁(σ)) ≡ 0 exactly");
    }

    /// The cylinder µ̂-form classifies inside/outside by sign, and its discriminant is positive
    /// exactly where the ruling crosses the cylinder — on the true surface, at a layer offset too.
    #[test]
    fn the_cylinder_mu_form_classifies_by_sign() {
        let chart = cone();
        // The demo D4 disk: a small vertical cylinder at (0, 11/5), R² = 1/25.
        let surface = CutSurface::Cylinder {
            axis_point: [Q::from_i128(0), Q::new(11, 5), Q::from_i128(0)],
            axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
            r2: Q::new(1, 25),
        };
        let form = cut_mu_form(&chart, &surface, &Q::from_i128(0)).unwrap();
        // At σ = 0 the +y ruling passes through the disk: the surface point at the certified
        // annulus band µ̂ ≈ 2.2 lies inside (s < 0); µ̂ = 1 is well inside the disk radially? No —
        // µ̂ = 1 sits at xy-radius ≈ 0.9 from the origin, far outside the disk (s > 0).
        let s_in = form.eval(&Q::from_i128(0), &Q::new(11, 5)).unwrap();
        let s_out = form.eval(&Q::from_i128(0), &Q::from_i128(1)).unwrap();
        // Corroborate the signs against the actual 3-D distance (exact arithmetic).
        let check = |mu: &Q, want_inside: bool| {
            let p = chart
                .surface(mu, &Q::from_i128(0))
                .eval(&Q::from_i128(0))
                .unwrap();
            let dy = p[1].sub(&Q::new(11, 5));
            let perp2 = p[0].mul(&p[0]).add(&dy.mul(&dy));
            assert_eq!(
                perp2.cmp(&Q::new(1, 25)) == core::cmp::Ordering::Less,
                want_inside
            );
        };
        assert!(s_in.sign() < 0, "µ̂ on the ruling chord is inside");
        check(&Q::new(11, 5), true);
        assert!(s_out.sign() > 0, "µ̂ = 1 is outside the disk");
        check(&Q::from_i128(1), false);
        // The discriminant: positive at σ = 0 (the ruling crosses the disk), negative at σ = 1
        // (azimuth 90° away — the ruling misses it).
        let disc = form.disc();
        assert!(disc.eval(&Q::from_i128(0)).unwrap().sign() > 0);
        assert!(disc.eval(&Q::from_i128(1)).unwrap().sign() < 0);
    }

    /// The cylinder metric: the certified ε upper-bounds the float distance-to-cylinder
    /// at sampled σ (ε is the sup, so it dominates every sample).
    #[test]
    fn cylinder_distance_upper_bounds_the_float_distance() {
        let chart = cone();
        // Cylinder about the z-axis through the origin, radius √(1/4) = 1/2.
        let axis_point = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(0)];
        let axis_dir = [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)];
        let r2 = Q::new(1, 4);
        // A simple linear rail μ̂(σ) = σ.
        let mu_hat = RatFunc::from_poly(Poly::from_coeffs(vec![Q::from_i128(0), Q::from_i128(1)]));
        let cert = CutFitCert {
            mu_hat: mu_hat.clone(),
            w: Q::from_i128(0),
            surface: CutSurface::Cylinder {
                axis_point,
                axis_dir,
                r2: r2.clone(),
            },
            span: ivl(1, 2),
            subdiv: 32,
            clearance: Q::from_i128(1000), // generous: we only read ε here
            cfg: DevConfig::tight(),
        };
        let eps = match cut_fit(&chart, &cert) {
            Verdict::Verified(v) => v.eps,
            other => panic!(
                "expected Verified (generous clearance), got {:?}",
                verdict_tag(&other)
            ),
        };
        // Float audit: at sampled σ, |√(Cx²+Cy²) − R| ≤ ε.
        let r = 0.5f64;
        for i in 0..=10 {
            let s = Q::new(100 + 10 * i, 100); // σ ∈ [1.0, 2.0]
            let mu = mu_hat.eval(&s).unwrap();
            let pt = chart.surface(&mu, &Q::from_i128(0)).eval(&s).unwrap();
            let (x, y) = (to_f64(&pt[0]), to_f64(&pt[1]));
            let dist = ((x * x + y * y).sqrt() - r).abs();
            assert!(
                dist <= to_f64(&eps) + 1e-9,
                "certified ε {} must dominate float dist {dist} at σ={}",
                to_f64(&eps),
                to_f64(&s)
            );
        }
    }

    // --- the extruded-cutter walls (AUTH.1a) --------------------------------------------------

    /// The demo D4 drill, authored twice: as the metric [`CutSurface::Cylinder`] it has always
    /// been, and as the wall of a profile circle extruded along `ẑ`.
    fn d4_two_ways() -> (CutSurface<Bignum>, CutSurface<Bignum>) {
        let centre = [Q::from_i128(0), Q::new(11, 5), Q::from_i128(0)];
        let r = Q::new(1, 5);
        let cyl = CutSurface::Cylinder {
            axis_point: centre.clone(),
            axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
            r2: r.mul(&r),
        };
        let wall = crate::extrude::ellipse_wall(
            &centre,
            &[r.clone(), Q::from_i128(0), Q::from_i128(0)],
            &[Q::from_i128(0), r.clone(), Q::from_i128(0)],
            &crate::extrude::Apex::direction([Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)])
                .unwrap(),
        )
        .expect("a real wall");
        (cyl, wall)
    }

    /// An extruded circle and the metric cylinder it *is* pull back to the **same** cut on the
    /// sheet — the two µ̂-quadratics agree up to the positive scale `r²`, which is no difference at
    /// all for a zero set. This is the differential check that the general wall did not quietly
    /// become a different surface than the special case it generalizes.
    #[test]
    fn an_extruded_circle_pulls_back_to_the_cylinder_it_is() {
        let chart = cone();
        let (cyl, wall) = d4_two_ways();
        let zero = Q::from_i128(0);
        let a = cut_mu_form(&chart, &cyl, &zero).unwrap();
        let b = cut_mu_form(&chart, &wall, &zero).unwrap();
        let r2 = RatFunc::from_poly(Poly::constant(Q::new(1, 25)));
        for (lhs, rhs) in [(&a.a, &b.a), (&a.b, &b.b), (&a.c, &b.c)] {
            assert!(
                lhs.mul(&r2).sub(rhs).reduce().is_zero(),
                "the extruded wall must pull back to r²·(the cylinder's own form)"
            );
        }
    }

    /// And the two agree as *certificates*: on the same rail, the general first-order bound lands
    /// within a small factor of the exact closed-form cylinder distance. The general path costs
    /// tightness, not soundness — and the factor is what says how much.
    #[test]
    fn the_first_order_bound_tracks_the_exact_distance() {
        let chart = cone();
        let (cyl, wall) = d4_two_ways();
        // A rail that genuinely traces the drill: the near branch of the µ̂-quadratic, sampled and
        // linearly interpolated, so it sits *close to* — not exactly on — the surface.
        let form = cut_mu_form(&chart, &cyl, &Q::from_i128(0)).unwrap();
        // The drill subtends `|φ| ≤ arcsin(1/11)` about the `+y` ruling, so with `φ = 2·arctan σ`
        // its σ-window is about `±0.045`. Stay well inside it: near a tangent ruling the branch
        // turns hard, and a chord across a wide span leaves the surface by a good fraction of the
        // drill's own radius — which is the regime the bound below is documented not to reach.
        let span = Interval {
            lo: Q::new(-1, 300),
            hi: Q::new(1, 300),
        };
        let cfg = DevConfig::tight();
        let at = |s: &Q| form.branch_at(s, &cfg.sqrt_eps).map(|(m, h)| m.sub(&h));
        let (m0, m1) = (at(&span.lo).unwrap(), at(&span.hi).unwrap());
        let slope = m1.sub(&m0).div(&span.hi.sub(&span.lo));
        let mu_hat =
            RatFunc::from_poly(Poly::from_coeffs(vec![m0.sub(&slope.mul(&span.lo)), slope]));
        let eps_of = |surface: CutSurface<Bignum>| {
            let cert = CutFitCert {
                mu_hat: mu_hat.clone(),
                w: Q::from_i128(0),
                surface,
                span: span.clone(),
                subdiv: 32,
                clearance: Q::from_i128(1), // generous: we only read ε here
                cfg: cfg.clone(),
            };
            match cut_fit(&chart, &cert) {
                Verdict::Verified(v) => v.eps,
                other => panic!("expected Verified, got {:?}", verdict_tag(&other)),
            }
        };
        let (exact, general) = (eps_of(cyl), eps_of(wall));
        assert!(
            general.cmp(&exact) != core::cmp::Ordering::Less,
            "the general bound must not undercut the exact distance: {} vs {}",
            to_f64(&general),
            to_f64(&exact)
        );
        assert!(
            general.cmp(&exact.mul(&Q::from_i128(4))) == core::cmp::Ordering::Less,
            "and must stay within a small factor of it: {} vs {}",
            to_f64(&general),
            to_f64(&exact)
        );
    }

    /// The first-order bound never *under*states a distance it certifies. Probed directly against
    /// exactly-known geometry: on the 45° cone `x² + y² = z²`, the point `(1 + δ, 0, 1)` is exactly
    /// `δ/√2` from the surface, and the certified bound brackets that from above.
    #[test]
    fn the_first_order_bound_is_sound_at_a_known_distance() {
        // `F = x² + y² − z²`, apex at the origin, authored nappe `z > 0`.
        let (o, i, j) = (Q::from_i128(0), Q::from_i128(1), Q::from_i128(-1));
        let m = [
            [i.clone(), o.clone(), o.clone()],
            [o.clone(), i.clone(), o.clone()],
            [o.clone(), o.clone(), j],
        ];
        let nappe = Nappe {
            n: [o.clone(), o.clone(), i],
            d: o.clone(),
        };
        let cfg = DevConfig::tight();
        for (dn, dd) in [(1i128, 100i128), (1, 1000), (1, 10_000)] {
            let delta = Q::new(dn, dd);
            let p = [
                Q::from_i128(1).add(&delta),
                Q::from_i128(0),
                Q::from_i128(1),
            ];
            let x: [RatIv<Bignum>; 3] = core::array::from_fn(|k| RatIv::point(p[k].clone()));
            let e = match quadric_distance_on(
                &m,
                &[o.clone(), o.clone(), o.clone()],
                &o,
                &nappe,
                &x,
                &Q::new(1, 10),
                &cfg,
            ) {
                DistOn::Bound(e) => e,
                _ => panic!("a point {} from the cone must certify", to_f64(&delta)),
            };
            // δ/√2 < δ·(707/1000) + a hair; use the safe rational bracket 7/10 < 1/√2 < 71/100.
            let lo = delta.mul(&Q::new(7, 10));
            let hi = delta.mul(&Q::new(71, 100)).mul(&Q::new(3, 2));
            assert!(
                e.cmp(&lo) != core::cmp::Ordering::Less,
                "bound {} undercuts the true distance ≈ {}",
                to_f64(&e),
                to_f64(&lo)
            );
            assert!(
                e.cmp(&hi) == core::cmp::Ordering::Less,
                "bound {} is far looser than the true distance ≈ {}",
                to_f64(&e),
                to_f64(&lo)
            );
        }
    }

    /// §4.1, fail-closed: a band on the **mirror** nappe carries a residual of zero — it is on the
    /// same double cone — and is refused anyway, because the cutter is one nappe. Nothing here is
    /// refinable, so the verdict is `Refuted`, not `Unresolved`.
    #[test]
    fn a_band_on_the_mirror_nappe_is_refuted() {
        let (o, i) = (Q::from_i128(0), Q::from_i128(1));
        let m = [
            [i.clone(), o.clone(), o.clone()],
            [o.clone(), i.clone(), o.clone()],
            [o.clone(), o.clone(), Q::from_i128(-1)],
        ];
        let nappe = Nappe {
            n: [o.clone(), o.clone(), i],
            d: o.clone(),
        };
        let cfg = DevConfig::tight();
        let probe = |z: i128| {
            let p = [Q::from_i128(z.abs()), Q::from_i128(0), Q::from_i128(z)];
            let x: [RatIv<Bignum>; 3] = core::array::from_fn(|k| RatIv::point(p[k].clone()));
            quadric_distance_on(
                &m,
                &[o.clone(), o.clone(), o.clone()],
                &o,
                &nappe,
                &x,
                &Q::new(1, 10),
                &cfg,
            )
        };
        assert!(
            matches!(probe(4), DistOn::Bound(_)),
            "a point on the authored nappe certifies"
        );
        assert!(
            matches!(probe(-4), DistOn::Fault(CutFitFault::NappeCrossed)),
            "and its mirror does not, though the residual is the same zero"
        );
        assert!(
            matches!(probe(0), DistOn::Fault(CutFitFault::NappeCrossed)),
            "nor does the apex, where inside inverts"
        );
    }

    /// **The draft actually drafts.** The same profile circle cast from four different heights
    /// cuts four different holes in the device cone, and each matches the taper law it claims:
    /// a cone from apex height `z_a` has shrunk to `|1 − z/z_a|` of its profile radius by height
    /// `z`, so the ruling's half-chord through it scales the same way. A cutter that merely *had*
    /// an apex field and ignored it would pass every certificate above and fail here.
    ///
    /// The drafted rail also certifies against its own wall, which is the end-to-end AUTH.1a
    /// claim: author with a cast point, pull back to `(σ, µ̂)`, certify.
    #[test]
    fn a_cast_point_tapers_the_hole_by_its_own_law() {
        let chart = cone();
        let zero = Q::from_i128(0);
        let cfg = DevConfig::tight();
        let centre = [zero.clone(), Q::new(11, 5), zero.clone()];
        let r = Q::new(1, 5);
        let e1 = [r.clone(), zero.clone(), zero.clone()];
        let e2 = [zero.clone(), r.clone(), zero.clone()];
        let wall = |apex: crate::extrude::Apex<Bignum>| {
            crate::extrude::ellipse_wall(&centre, &e1, &e2, &apex).expect("a real wall")
        };
        let half_width = |surface: &CutSurface<Bignum>| {
            cut_mu_form(&chart, surface, &zero)
                .unwrap()
                .branch_at(&zero, &cfg.sqrt_eps)
                .expect("the +y ruling crosses the drill")
        };

        let straight = wall(
            crate::extrude::Apex::direction([zero.clone(), zero.clone(), Q::from_i128(1)]).unwrap(),
        );
        let (m_par, h_par) = half_width(&straight);
        // The height the cut sits at — the taper law's only input besides the apex.
        let z = {
            let p = chart.pedal().eval(&zero).unwrap();
            let u = chart.ruling().eval(&zero).unwrap();
            p[2].add(&u[2].mul(&m_par))
        };
        assert!(
            z.sign() > 0,
            "the fixture's cut must sit above the profile plane"
        );

        let mut widths = Vec::new();
        for za in [4i128, 40, -40, -4] {
            let apex_z = Q::from_i128(za);
            let cone_wall = wall(crate::extrude::Apex::point([
                zero.clone(),
                Q::new(11, 5),
                apex_z.clone(),
            ]));
            let (m, h) = half_width(&cone_wall);
            // The taper law: `radius(z) = r·|1 − z/z_a|`, so the half-chord scales with it.
            let scale = Q::from_i128(1).sub(&z.div(&apex_z));
            let want = h_par.mul(&abs_rat(&scale));
            let slack = h_par.mul(&Q::new(1, 50)); // 2%: the chord is not exactly radial
            assert!(
                abs_rat(&h.sub(&want)).cmp(&slack) == core::cmp::Ordering::Less,
                "apex z={za}: half-width {} but the taper law says {}",
                to_f64(&h),
                to_f64(&want)
            );
            widths.push(h.clone());

            // ... and the drafted rail certifies against its own wall.
            let span = Interval {
                lo: Q::new(-1, 300),
                hi: Q::new(1, 300),
            };
            let cert = CutFitCert {
                mu_hat: RatFunc::from_poly(Poly::constant(m.sub(&h))),
                w: zero.clone(),
                surface: cone_wall,
                span,
                subdiv: 32,
                clearance: Q::new(1, 20),
                cfg: cfg.clone(),
            };
            match cut_fit(&chart, &cert) {
                Verdict::Verified(v) => assert!(
                    v.eps.cmp(&Q::new(1, 100)) == core::cmp::Ordering::Less,
                    "apex z={za}: ε = {}",
                    to_f64(&v.eps)
                ),
                other => panic!(
                    "apex z={za}: expected Verified, got {:?}",
                    verdict_tag(&other)
                ),
            }
        }
        // Ordered by `1/z_a` from `+1/4` to `−1/4`: the hole widens monotonically as the cast point
        // swings from just above the cut, out to infinity, and round to just below it.
        for pair in widths.windows(2) {
            assert!(
                pair[0].cmp(&pair[1]) == core::cmp::Ordering::Less,
                "the taper must be monotone in the cast point"
            );
        }
        assert!(
            widths[0].cmp(&h_par) == core::cmp::Ordering::Less
                && h_par.cmp(&widths[3]) == core::cmp::Ordering::Less,
            "and must bracket the parallel drill"
        );
    }

    fn verdict_tag<E, W: core::fmt::Debug, M>(v: &Verdict<E, W, M>) -> String {
        match v {
            Verdict::Verified(_) => "Verified".into(),
            Verdict::Refuted(w) => format!("Refuted({w:?})"),
            Verdict::Unresolved(_) => "Unresolved".into(),
        }
    }
}
