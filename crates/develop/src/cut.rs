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

/// A [`Quadric`] recognized as a **cone of revolution**, in the metric parametrization its
/// distance has a closed form in.
///
/// The general quadric arm bounds the distance to `{F = 0}` by inflating a box around the traced
/// point until a first-order bound closes ([`quadric_distance_on`]). That is the honest thing to do
/// for a surface with no metric parametrization — but a *circular* cone has one, and then the
/// distance is as elementary as a cylinder's: in the meridian half-plane through the point, the
/// nappe is the ray from the apex at angle α to the axis, and the distance to a ray is either the
/// perpendicular drop or the distance to its endpoint. Recognizing the case is worth a great deal,
/// because it moves the certificate from a box bound (which loses the σ↔µ̂ correlation, and so
/// needs a far finer split for the same ε) to a **symbolic residual in σ**, which does not.
///
/// A normal cut — the disc cutter whose generatrix runs along the sheet's own normal — is exactly
/// this case, and it is the cut a physical trim actually makes.
///
/// Recognition is a *verified* proposal, not a classification: [`RevCone::recognize`] extracts the
/// candidate `(apex, axis, α)` and then checks the equality `F ≡ (X−p)ᵀS(X−p)` exactly over ℚ.
/// Nothing is certified on a guess, and anything that fails falls back to the general arm.
#[derive(Clone)]
pub struct RevCone<B: Backend = Bignum> {
    /// The apex.
    pub apex: [Rat<B>; 3],
    /// The axis direction — **not** unit, and oriented into the authored [`Nappe`].
    pub axis: [Rat<B>; 3],
    /// `cos²α` for the half-angle `α`, strictly between `0` and `1`. Squared because that is what
    /// stays rational: `72/65` is a rational *slope*, never a rational cosine.
    pub cos2: Rat<B>,
}

impl<B: Backend> RevCone<B> {
    /// Recognize `q` as a cone of revolution, or `None`.
    ///
    /// The extraction is linear algebra over ℚ on the symmetric part `S`. A cone of revolution has
    /// `S = r·I + ν·a aᵀ` — eigenvalue `r` twice across the axis, once along it — so:
    ///
    /// 1. `r` is the **double root** of the characteristic cubic, and a double root of a rational
    ///    cubic is rational (an irrational one would drag its conjugate in and need degree 4), so
    ///    the closed form `(e₁e₂ − 9e₃)/(2(e₁² − 3e₂))` gives it without any root-finding;
    /// 2. `S − r·I` must then be exactly **rank one**, which pins the axis as any nonzero column;
    /// 3. the apex solves `S·p = −b/2`, and the constant must come out at `c = pᵀSp`.
    ///
    /// Each step is checked, not assumed: the rank-one identity, the apex solve and the constant
    /// are exact rational equalities, and `cos²α ∈ (0, 1)` rejects the degenerate ends (a line at
    /// `α = 0`, a plane pair at `α = 90°`). An elliptic cone fails step 2; a hyperboloid fails
    /// step 3; a **cylinder** of revolution passes steps 1 and 2 — its eigenvalues are `λ, λ, 0`, so
    /// the double root is `λ`, not `0` — and fails at `cos²α = 1`, which is the right place for it:
    /// a cylinder is the `α → 0` limit with the apex gone to infinity. [`RevCylinder`] is its
    /// recognizer, and the two are exhaustive over surfaces of revolution a sketch can sweep.
    pub fn recognize(q: &Quadric<B>) -> Option<Self> {
        let half = Rat::new(1, 2);
        let s: [[Rat<B>; 3]; 3] = core::array::from_fn(|i| {
            core::array::from_fn(|j| q.m[i][j].add(&q.m[j][i]).mul(&half))
        });

        // det(S − t·I) = −t³ + e₁t² − e₂t + e₃.
        let e1 = s[0][0].add(&s[1][1]).add(&s[2][2]);
        let minor = |i: usize, j: usize| s[i][i].mul(&s[j][j]).sub(&s[i][j].mul(&s[j][i]));
        let e2 = minor(0, 1).add(&minor(0, 2)).add(&minor(1, 2));
        let den = e1.mul(&e1).sub(&e2.mul(&Rat::from_i128(3)));
        if den.is_zero() {
            // A triple root: `S` is a multiple of the identity — a sphere or a point, not a cone.
            return None;
        }
        let e3 = det3(&s);
        let r = e1
            .mul(&e2)
            .sub(&e3.mul(&Rat::from_i128(9)))
            .div(&den.mul(&Rat::from_i128(2)));
        if r.is_zero() {
            // The across-axis eigenvalue is the half-angle; zero makes `S` singular, which is a
            // cylinder (or worse), not a cone with an apex.
            return None;
        }

        // `S − r·I = a aᵀ / w` exactly, with `a` a nonzero column and `w` its diagonal entry. The
        // identity is checked on every entry — it is what makes the rest sound.
        let rr: [[Rat<B>; 3]; 3] = core::array::from_fn(|i| {
            core::array::from_fn(|j| {
                if i == j {
                    s[i][j].sub(&r)
                } else {
                    s[i][j].clone()
                }
            })
        });
        let j = (0..3).find(|&j| !rr[j][j].is_zero())?;
        let w = rr[j][j].clone();
        let a: [Rat<B>; 3] = core::array::from_fn(|i| rr[i][j].clone());
        for (i, row) in rr.iter().enumerate() {
            for (l, entry) in row.iter().enumerate() {
                if !entry.mul(&w).sub(&a[i].mul(&a[l])).is_zero() {
                    return None;
                }
            }
        }

        // The along-axis eigenvalue exceeds `r` by `Λ = |a|²/w`, and `cos²α = −r/Λ`.
        let a2 = dot3(&a, &a);
        let cos2 = r.neg().mul(&w).div(&a2);
        if cos2.sign() <= 0 || cos2.sub(&Rat::from_i128(1)).sign() >= 0 {
            return None;
        }

        // The apex kills the linear term, and the constant must then vanish with it.
        let rhs: [Rat<B>; 3] = core::array::from_fn(|i| q.b[i].mul(&half).neg());
        let apex = solve3(&s, &rhs)?;
        if !q.c.add(&dot3(&q.b, &apex).mul(&half)).is_zero() {
            return None;
        }

        // Which of the two nappes is the authored one. The selector plane passes through the apex
        // and strictly separates them, so the sign of `n·a` names the nappe — and `n·a = 0` (a
        // cylinder's vacuous selector, or a selector that cannot tell them apart) declines.
        let sgn = dot3(&q.nappe.n, &a).sign();
        if sgn == 0 {
            return None;
        }
        let axis = if sgn > 0 {
            a
        } else {
            core::array::from_fn(|i| a[i].neg())
        };
        Some(RevCone { apex, axis, cos2 })
    }

    /// `√((1 − cos²α)/|axis|²)` — the constant the distance formula scales `(X−p)·axis` by, so that
    /// `t̂·sin α` needs no unit axis. Enclosed once per certificate, not once per sub-interval.
    fn sin_scaled(&self, cfg: &DevConfig<B>) -> RatIv<B> {
        let a2 = dot3(&self.axis, &self.axis);
        sqrt(&Rat::from_i128(1).sub(&self.cos2).div(&a2), &cfg.sqrt_eps)
    }

    /// An upper bound on the distance from an enclosed point to the **authored nappe**, given the
    /// three scalars the formula needs already enclosed: `n2 = |X−p|²`, `ta = (X−p)·axis`, and
    /// `s2 = n2 − ta²/|axis|²` (the squared distance to the axis).
    ///
    /// With `t̂ = ta/|axis|` and `s = √s2` the meridian coordinates, the nappe is the ray
    /// `{(t̂, s) : s = t̂·tan α, t̂ ≥ 0}`, whose distance is the perpendicular drop
    /// `|s·cos α − t̂·sin α|` while the foot stays on the ray — which `t̂ ≥ 0` guarantees, since
    /// `s ≥ 0` makes the projection at least `t̂·cos α`. Where the enclosure cannot rule out
    /// `t̂ < 0` the nearest point may be the apex, and `|X − p|` bounds *that* case and every other
    /// one at once (the apex is on the ray), so the fallback is sound rather than a refusal.
    ///
    /// Both radicands stay rational — `k·s2` and `((1−k)/|axis|²)·ta²` — which is the whole point:
    /// the irrational half-angle never has to be represented.
    fn dist_hi(
        &self,
        n2: &RatIv<B>,
        ta: &RatIv<B>,
        s2: &RatIv<B>,
        sin_scaled: &RatIv<B>,
        cfg: &DevConfig<B>,
    ) -> Rat<B> {
        if ta.lo().sign() < 0 {
            return sqrt_on(n2, &cfg.sqrt_eps).hi().clone();
        }
        let perp = sqrt_on(&s2.mul(&RatIv::point(self.cos2.clone())), &cfg.sqrt_eps)
            .sub(&sin_scaled.mul(ta))
            .rounded();
        abs_on(&perp).hi().clone()
    }
}

/// A [`Quadric`] recognized as a **cylinder of revolution** — the same move [`RevCone`] makes, for
/// the apex that went to infinity.
///
/// A sketch swept from a *direction* rather than a point (`Apex::direction`, the straight drill) is
/// one of the two apex kinds any profile can be cut with, and its circle clears to a
/// [`CutSurface::Quadric`] just as a finite apex's does. The general quadric arm then bounds
/// distance by inflating a box — which measured **ε 5.56 against 1.53** for the same Ø 8 bore on the
/// acceptance device, purely as a change of instrument. `CutSurface::Cylinder` has had the closed
/// form all along; all that was missing was noticing that the quadric *is* one. Without this, the
/// two apex kinds cost seven orders of magnitude differently for no geometric reason.
///
/// Like [`RevCone`], recognition is a verified proposal: every step below is an exact rational
/// equality, and anything that fails falls back to the general arm.
#[derive(Clone)]
pub struct RevCylinder<B: Backend = Bignum> {
    /// A point on the axis — any one; the axis is a line, and the certificate only needs it as a
    /// line.
    pub axis_point: [Rat<B>; 3],
    /// The axis direction, **not** unit.
    pub axis_dir: [Rat<B>; 3],
    /// The squared radius.
    pub r2: Rat<B>,
}

impl<B: Backend> RevCylinder<B> {
    /// Recognize `q` as a cylinder of revolution, or `None`.
    ///
    /// A cylinder's symmetric part is `S = λ·(I − ââᵀ/|â|²)` — eigenvalues `λ, λ, 0` — so:
    ///
    /// 1. `det S = 0`, and the double root of the characteristic cubic is `λ ≠ 0` (the same closed
    ///    form [`RevCone::recognize`] uses, with `e₃ = 0`);
    /// 2. `N := λI − S` must be exactly **rank one**, which pins the axis direction as any nonzero
    ///    column `a`, and `|a|² = λ·N_jj` pins the scale — together those two say `S` is a multiple
    ///    of the projector orthogonal to `a`, and nothing else;
    /// 3. `S` is singular, so the axis is a *line* of solutions of `S·p = −b/2` rather than a point.
    ///    `p = −b/(2λ)` is the one on the plane through the origin normal to the axis, and checking
    ///    `b + 2S·p = 0` is what rejects a parabolic cylinder, whose `b` has a component along `a`
    ///    that no `p` can absorb;
    /// 4. `R² = −(b·p/2 + c)/λ` must come out strictly positive — an imaginary or degenerate
    ///    cylinder is not a surface a cut can lie on.
    ///
    /// **The selector must be vacuous.** A quadric carrying a live [`Nappe`] describes *half* a
    /// surface, and the distance to a whole cylinder is not an upper bound for the distance to half
    /// of one — it is a lower bound, which is the unsound direction. A direction apex sets
    /// `τ = −w·N = 0`, so `nappe.n = 0` is exactly the case with no second sheet to choose, and it
    /// is the only case accepted here.
    pub fn recognize(q: &Quadric<B>) -> Option<Self> {
        if q.nappe.n.iter().any(|c| !c.is_zero()) {
            return None;
        }
        let half = Rat::new(1, 2);
        let s: [[Rat<B>; 3]; 3] = core::array::from_fn(|i| {
            core::array::from_fn(|j| q.m[i][j].add(&q.m[j][i]).mul(&half))
        });

        if !det3(&s).is_zero() {
            return None; // no zero eigenvalue: not a cylinder
        }
        let e1 = s[0][0].add(&s[1][1]).add(&s[2][2]);
        let minor = |i: usize, j: usize| s[i][i].mul(&s[j][j]).sub(&s[i][j].mul(&s[j][i]));
        let e2 = minor(0, 1).add(&minor(0, 2)).add(&minor(1, 2));
        let den = e1.mul(&e1).sub(&e2.mul(&Rat::from_i128(3)));
        if den.is_zero() {
            return None; // a triple root, i.e. `S = 0`: a plane or nothing
        }
        let lam = e1.mul(&e2).div(&den.mul(&Rat::from_i128(2)));
        if lam.is_zero() {
            return None;
        }

        // `N = λI − S` is `λ·a aᵀ/|a|²`: rank one, and of that one scale.
        let n: [[Rat<B>; 3]; 3] = core::array::from_fn(|i| {
            core::array::from_fn(|j| {
                if i == j {
                    lam.sub(&s[i][j])
                } else {
                    s[i][j].neg()
                }
            })
        });
        let j = (0..3).find(|&j| !n[j][j].is_zero())?;
        let w = n[j][j].clone();
        let a: [Rat<B>; 3] = core::array::from_fn(|i| n[i][j].clone());
        for (i, row) in n.iter().enumerate() {
            for (l, entry) in row.iter().enumerate() {
                if !entry.mul(&w).sub(&a[i].mul(&a[l])).is_zero() {
                    return None;
                }
            }
        }
        if !dot3(&a, &a).sub(&lam.mul(&w)).is_zero() {
            return None;
        }

        // The axis point, and the check that the linear term is one an axis can absorb.
        let p: [Rat<B>; 3] =
            core::array::from_fn(|i| q.b[i].neg().div(&lam.mul(&Rat::from_i128(2))));
        for (row, bi) in s.iter().zip(&q.b) {
            let sp = row
                .iter()
                .zip(&p)
                .fold(Rat::from_i128(0), |acc, (m, pk)| acc.add(&m.mul(pk)));
            if !bi.add(&sp.mul(&Rat::from_i128(2))).is_zero() {
                return None;
            }
        }

        // `F(p) = −λR²`, and `pᵀSp = −b·p/2` because `S·p = −b/2`.
        let r2 = dot3(&q.b, &p).mul(&half).add(&q.c).neg().div(&lam);
        if r2.sign() <= 0 {
            return None;
        }
        Some(RevCylinder {
            axis_point: p,
            axis_dir: a,
            r2,
        })
    }

    /// The equivalent [`CutSurface::Cylinder`] — the same point set, in the representation whose
    /// distance is a symbolic residual in σ.
    pub fn as_cut_surface(&self) -> CutSurface<B> {
        CutSurface::Cylinder {
            axis_point: self.axis_point.clone(),
            axis_dir: self.axis_dir.clone(),
            r2: self.r2.clone(),
        }
    }
}

/// The determinant of an exact 3×3.
fn det3<B: Backend>(m: &[[Rat<B>; 3]; 3]) -> Rat<B> {
    let cof = |i: usize, j: usize, k: usize, l: usize| m[i][j].mul(&m[k][l]);
    m[0][0]
        .mul(&cof(1, 1, 2, 2).sub(&cof(1, 2, 2, 1)))
        .sub(&m[0][1].mul(&cof(1, 0, 2, 2).sub(&cof(1, 2, 2, 0))))
        .add(&m[0][2].mul(&cof(1, 0, 2, 1).sub(&cof(1, 1, 2, 0))))
}

/// The exact solution of `M·x = rhs` by Cramer's rule. `None` when `M` is singular.
fn solve3<B: Backend>(m: &[[Rat<B>; 3]; 3], rhs: &[Rat<B>; 3]) -> Option<[Rat<B>; 3]> {
    let d = det3(m);
    if d.is_zero() {
        return None;
    }
    Some(core::array::from_fn(|c| {
        let mut sub: [[Rat<B>; 3]; 3] = m.clone();
        for (row, r) in sub.iter_mut().zip(rhs) {
            row[c] = r.clone();
        }
        det3(&sub).div(&d)
    }))
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
    /// The cutter is not an **interior** hole over this window: it swallows a whole ruling, or its
    /// footprint reaches the window's own edge instead of closing inside it. Either way there is no
    /// closed loop to build here — widen the window, or author the cut as a boundary op.
    ShadowUnbounded,
    /// The cutter's own fill rule could not be read at a sample, even after the genericity nudge
    /// ([`Cast::contains`](crate::extrude::Cast::contains) returning `None` at every offset tried).
    /// A refusal, not a guess.
    ShadowUndecided,
    /// Two or more stretches merged **and** split inside one event bracket, so which boundary end
    /// continues into which is not determined by the sweep. Refine: a narrower event tolerance
    /// separates the events, and each one alone is an ordinary merge or split.
    ShadowEventTangled,
    /// One traced loop lies **inside** another: the footprint has a hole of its own, so the cut
    /// would leave an island of material floating free — two parts, not one hole
    /// (`docs/cutter-extrude-design.md` §11.6). A ring profile lands here, by name.
    ShadowNested,
    /// A traced boundary did not close: some vertex carried other than the two rails a boundary
    /// vertex must have. A refusal rather than a repair — an open loop is exactly what the flat
    /// boolean would stitch into something else.
    ShadowLoopOpen,
    /// A [`structure_events`] Sturm chain failed its own runtime hypothesis check
    /// ([`SturmChain::verify_chain`]). The root count it would give is not one to be trusted, so
    /// the event set is refused rather than taken on faith — the same discipline
    /// [`crate::pick::ray_crossings`] applies to the span's crossing count.
    EventChainUnverified,
}

/// Which structural change a [`StructureEvent`] is — the three ways a ruling's stretch structure
/// can change (`docs/cutter-extrude-design.md` §11.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EventKind {
    /// `disc_µ̂(f_i) = 0`: wall `i`'s own two crossings collide. A stretch is born or dies — §10's
    /// tangent ruling, now one class among others rather than the two ends of everything.
    Tangent(usize),
    /// `Res_µ̂(f_i, f_j) = 0`: walls `i` and `j` cross the ruling at the same µ̂. This is the
    /// merge/split saddle where two stretches coalesce **and** the governing-wall corner of §10.2 —
    /// one event seen from two sides.
    Meet(usize, usize),
    /// `a_i(σ) = 0`: wall `i`'s form degenerates from a conic to a line in µ̂ and one crossing
    /// escapes to infinity. Nothing *meets*, which is what makes it easy to miss, but the stretch
    /// count changes all the same.
    Escape(usize),
}

/// One σ-bracket in which the ruling's stretch structure changes, and the wall(s) responsible.
///
/// The bracket is what the tracer partitions on: cells are the gaps *between* brackets, where the
/// stretch count is constant, so a bracket is treated as a thin event zone rather than a point.
pub struct StructureEvent<B: Backend = Bignum> {
    /// A bracket containing the event σ (or several, if two events were closer than `tol`).
    pub at: Interval<B>,
    /// Every class that put an event in this bracket, in discovery order.
    pub kinds: Vec<EventKind>,
}

impl<B: Backend> MuCut<B> {
    /// The **resultant** `Res_µ̂(self, other)` — zero exactly at the σ where the two walls cross the
    /// ruling at a common µ̂ (`docs/cutter-extrude-design.md` §11.2).
    ///
    /// Taken at each form's **actual** µ̂-degree, which is a correctness requirement and not a
    /// tidiness one. The quadratic-by-quadratic closed form
    /// `(a₁c₂ − a₂c₁)² − (a₁b₂ − a₂b₁)(b₁c₂ − b₂c₁)` is the 4×4 Sylvester determinant of the two
    /// forms *padded to degree 2*, and padding a genuinely affine form adds a shared root at
    /// infinity: with **both** walls affine — every wall of a polygonal profile, so the L-slot this
    /// milestone is for — it collapses to `0` for meeting and non-meeting walls alike. So:
    ///
    /// | degrees | resultant |
    /// |---|---|
    /// | 2 × 2 | `(a₁c₂ − a₂c₁)² − (a₁b₂ − a₂b₁)(b₁c₂ − b₂c₁)` |
    /// | 2 × 1 | `a₁c₂² − b₁b₂c₂ + c₁b₂²` (the conic evaluated at the line's root, cleared) |
    /// | 1 × 1 | `b₁c₂ − b₂c₁` |
    ///
    /// The dispatch is on `a ≡ 0` as a *rational function* — a plane wall, decidable and static.
    /// An isolated σ where a genuine conic's `a(σ)` vanishes needs no special case: the 2 × 2 form
    /// there factors as `a_j·(a_j c_i² + b_i² c_j − b_i b_j c_i)`, whose vanishing (given `a_j ≠ 0`)
    /// is exactly the 2 × 1 condition. Those σ are also [`EventKind::Escape`] events in their own
    /// right.
    pub fn resultant(&self, other: &MuCut<B>) -> RatFunc<B> {
        // `f` quadratic, `g` affine: `b_g²·f(−c_g/b_g)`, denominator cleared.
        let mixed = |f: &MuCut<B>, g: &MuCut<B>| {
            f.a.mul(&g.c)
                .mul(&g.c)
                .sub(&f.b.mul(&g.b).mul(&g.c))
                .add(&f.c.mul(&g.b).mul(&g.b))
        };
        match (self.a.is_zero(), other.a.is_zero()) {
            (true, true) => self.b.mul(&other.c).sub(&other.b.mul(&self.c)),
            (false, true) => mixed(self, other),
            (true, false) => mixed(other, self),
            (false, false) => {
                let minor = |p: &RatFunc<B>, q: &RatFunc<B>, r: &RatFunc<B>, s: &RatFunc<B>| {
                    p.mul(s).sub(&q.mul(r)) // det [[p, q], [r, s]]
                };
                let ac = minor(&self.a, &self.c, &other.a, &other.c);
                let ab = minor(&self.a, &self.b, &other.a, &other.b);
                let bc = minor(&self.b, &self.c, &other.b, &other.c);
                ac.mul(&ac).sub(&ab.mul(&bc))
            }
        }
    }
}

/// Every σ in `window` at which the stretch structure of the ruling can change, as disjoint
/// brackets in increasing order — the exact event set the AUTH.2 tracer sweeps over
/// (`docs/cutter-extrude-design.md` §11.2).
///
/// Three polynomial families, one per [`EventKind`]: each wall's discriminant, each pair's
/// [`resultant`](MuCut::resultant), and each wall's leading coefficient. All are rational functions
/// of σ, so their roots are the roots of their numerators, isolated by `lattice`'s Sturm chain —
/// which counts **distinct** roots even when a polynomial is not squarefree, so a double event (a
/// tangential touch rather than a transverse crossing) is located rather than refused.
///
/// Each bracket is bisected until it is narrower than `tol`; brackets that still overlap are merged
/// and carry both kinds, so the result partitions `window` no matter how close two events sit.
///
/// **This is a tightness device, not a soundness one.** A σ this misses — two events inside one
/// `tol`-wide bracket, say — costs the tracer accuracy, and the σ-midpoint comparison against the
/// fill rule is what keeps the emitted boundary honest regardless (§11.5). Erring toward *more*
/// brackets is therefore free, which is why nothing here works to suppress a spurious root.
///
/// `Err` only if a Sturm chain fails its own hypothesis check ([`CutFitFault::EventChainUnverified`]).
///
/// ```
/// use develop::cut::{EventKind, MuCut, structure_events};
/// use lattice::{Bignum, Interval, Poly, Rat, RatFunc};
///
/// type Q = Rat<Bignum>;
/// let poly = |c: &[i128]| {
///     RatFunc::from_poly(Poly::from_coeffs(c.iter().map(|v| Q::from_i128(*v)).collect()))
/// };
/// // Two walls of a polygonal profile, as µ̂-forms: `µ̂ = σ` and `µ̂ = 1 − σ`.
/// let walls = [
///     MuCut { a: poly(&[]), b: poly(&[1]), c: poly(&[0, -1]) },
///     MuCut { a: poly(&[]), b: poly(&[1]), c: poly(&[-1, 1]) },
/// ];
/// let window = Interval { lo: Q::from_i128(0), hi: Q::from_i128(1) };
/// let events = structure_events(&walls, &window, &Q::new(1, 1024)).unwrap();
///
/// // They cross the ruling at a common µ̂ at σ = 1/2 — a profile corner, bracketed exactly.
/// assert_eq!(events.len(), 1);
/// assert_eq!(events[0].kinds, vec![EventKind::Meet(0, 1)]);
/// assert!(events[0].at.lo <= Q::new(1, 2) && Q::new(1, 2) <= events[0].at.hi);
/// ```
pub fn structure_events<B: Backend>(
    forms: &[MuCut<B>],
    window: &Interval<B>,
    tol: &Rat<B>,
) -> Result<Vec<StructureEvent<B>>, CutFitFault> {
    use core::cmp::Ordering;

    let mut found: Vec<(Interval<B>, EventKind)> = Vec::new();
    let mut collect = |rf: &RatFunc<B>, kind: EventKind| -> Result<(), CutFitFault> {
        for iv in isolate_roots(rf, window, tol)? {
            found.push((iv, kind));
        }
        Ok(())
    };

    for (i, f) in forms.iter().enumerate() {
        collect(&f.disc(), EventKind::Tangent(i))?;
        collect(&f.a, EventKind::Escape(i))?;
        for (j, g) in forms.iter().enumerate().skip(i + 1) {
            collect(&f.resultant(g), EventKind::Meet(i, j))?;
        }
    }

    found.sort_by(|x, y| x.0.lo.cmp(&y.0.lo));
    let mut out: Vec<StructureEvent<B>> = Vec::with_capacity(found.len());
    for (iv, kind) in found {
        match out.last_mut() {
            // Overlapping (or touching) brackets become one event zone carrying both kinds — the
            // partition must stay a partition even where two events are closer than `tol`.
            Some(prev) if iv.lo.cmp(&prev.at.hi) != Ordering::Greater => {
                if iv.hi.cmp(&prev.at.hi) == Ordering::Greater {
                    prev.at.hi = iv.hi;
                }
                prev.kinds.push(kind);
            }
            _ => out.push(StructureEvent {
                at: iv,
                kinds: vec![kind],
            }),
        }
    }
    Ok(out)
}

/// The isolating brackets of a rational function's roots in `window`, in σ order, each bisected
/// until narrower than `tol` — the exact root locator every event family is built from.
///
/// Roots of a rational function are roots of its numerator, isolated by `lattice`'s Sturm chain,
/// which counts **distinct** roots even when the polynomial is not squarefree — so a double root (a
/// tangential touch rather than a transverse crossing) is located rather than stepped over. The
/// brackets are disjoint and each holds exactly one root, which is what lets a caller treat the gaps
/// between them as intervals of constant sign.
///
/// `Err` only if a Sturm chain fails its own hypothesis check ([`CutFitFault::EventChainUnverified`]).
fn isolate_roots<B: Backend>(
    rf: &RatFunc<B>,
    window: &Interval<B>,
    tol: &Rat<B>,
) -> Result<Vec<Interval<B>>, CutFitFault> {
    use core::cmp::Ordering;
    use lattice::SturmChain;
    /// Bisection cap per bracket — 2⁻⁶⁴ of the starting width, well past any usable `tol`.
    const MAX_BISECT: usize = 64;

    // Reduce before isolating, and it is not a micro-optimization: these families are products
    // of the chart's own rational fields, so they arrive carrying the chart denominator several
    // times over. On the AUTH.1e.4 square prism a raw pairwise resultant is **degree 78** and
    // its reduced form is **degree 4** — the difference between a naive ℚ-PRS Sturm chain over
    // 78 coefficients and one over 4, measured at 273 ms → 16 ms for the whole event set. The
    // cancelled factors are shared with the denominator, so their roots are removable
    // singularities rather than events; dropping them is also the more honest partition.
    let reduced = rf.reduce();
    let p = reduced.num();
    // An identically-zero family (duplicate walls, say) has no *isolated* root, and a nonzero
    // constant has no root at all. Neither is a fault: the fill rule still decides membership.
    if p.is_zero() || p.degree().unwrap_or(0) == 0 {
        return Ok(Vec::new());
    }
    let chain = SturmChain::new(p);
    if !chain.verify_chain(p) {
        return Err(CutFitFault::EventChainUnverified);
    }
    let mut out = Vec::new();
    for iv in chain.isolate(window) {
        // Narrow by bisection on the Sturm *count*, not on a sign change: an even-multiplicity
        // root never flips sign, and those are exactly the tangential events worth locating.
        let (mut lo, mut hi) = (iv.lo, iv.hi);
        for _ in 0..MAX_BISECT {
            if hi.sub(&lo).cmp(tol) != Ordering::Greater {
                break;
            }
            let mid = lo.add(&hi).mul(&Rat::new(1, 2));
            if chain.count_in(&lo, &mid) > 0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        out.push(Interval { lo, hi });
    }
    Ok(out)
}

/// One wall's **tangent rulings** over `window`: the σ where its µ̂-pullback's discriminant
/// vanishes, as disjoint isolating brackets in σ order, each narrower than `tol`.
///
/// A wall whose pullback is a genuine quadratic carries material only where the discriminant is
/// positive, so these brackets delimit its σ-windows: between consecutive brackets the discriminant
/// has **constant sign**, and one evaluation anywhere in the gap decides whether that gap is a
/// window. That is the property a sign-change scan cannot offer — a window narrower than one scan
/// cell puts both of its roots in the same cell, the scan sees no sign change, and the wall is
/// reported as never touching the material at all (the fail-*open* direction: a cut that was never
/// derived leaves no trace in the flat pattern, the solid, or ε). This is [`structure_events`]'
/// `Tangent` family on a single form, which is what the resolver needs when it is asking about one
/// wall rather than about a profile's whole stretch structure.
///
/// ```
/// use develop::cut::{MuCut, tangent_events};
/// use lattice::{Bignum, Interval, Poly, Rat, RatFunc};
///
/// type Q = Rat<Bignum>;
/// let poly = |c: &[i128]| {
///     RatFunc::from_poly(Poly::from_coeffs(c.iter().map(|v| Q::from_i128(*v)).collect()))
/// };
/// // `µ̂² + σ² − 1 = 0`: real between the tangent rulings σ = ±1, and nowhere else.
/// let wall = MuCut { a: poly(&[1]), b: poly(&[]), c: poly(&[-1, 0, 1]) };
/// let window = Interval { lo: Q::from_i128(-2), hi: Q::from_i128(2) };
/// let ts = tangent_events(&wall, &window, &Q::new(1, 1 << 20)).unwrap();
/// assert_eq!(ts.len(), 2);
/// assert!(ts[0].lo <= Q::from_i128(-1) && Q::from_i128(-1) <= ts[0].hi);
/// assert!(ts[1].lo <= Q::from_i128(1) && Q::from_i128(1) <= ts[1].hi);
/// ```
pub fn tangent_events<B: Backend>(
    form: &MuCut<B>,
    window: &Interval<B>,
    tol: &Rat<B>,
) -> Result<Vec<Interval<B>>, CutFitFault> {
    // Sorted here rather than relied on: `SturmChain::isolate` returns the brackets in its own
    // recursion order (the `+1` root before the `−1` one on the doctest above), and a caller
    // reading consecutive pairs as windows needs σ order. `structure_events` sorts for the same
    // reason, one step later, after merging its three families.
    let mut out = isolate_roots(&form.disc(), window, tol)?;
    out.sort_by(|a, b| a.lo.cmp(&b.lo));
    Ok(out)
}

/// One wall's **coverage flips**: the σ where a wall that is *degenerate in µ̂* (`a ≡ b ≡ 0` — a
/// plane containing the whole ruling family, which happens on a cylinder chart) switches between
/// covering the entire ruling and covering none of it, as disjoint isolating brackets in σ order.
///
/// This is the third way a ruling's stretch structure can change, and it is the one the other two
/// families cannot see. [`structure_events`]' `Tangent` reads `disc = b² − 4ac`, which for such a
/// wall is identically zero; `Escape` reads `a`, also identically zero. Nothing *meets* and nothing
/// *escapes* — the shadow simply flips between `Patch::All` and empty at a root of `c`. Where the
/// other families bound material by a **pinch** (the two bounding rails converge), this one bounds
/// it by a **jump**: the material ends at full width.
///
/// Returns the empty vector for any wall that is not degenerate, so a caller can fold it over every
/// wall unconditionally. Kept separate from [`structure_events`] on purpose: adding a family there
/// would refine the tracer's σ-partition on charts where it is not needed.
///
/// ```
/// use develop::cut::{MuCut, coverage_events};
/// use lattice::{Bignum, Interval, Poly, Rat, RatFunc};
///
/// type Q = Rat<Bignum>;
/// let poly = |cs: &[i128]| {
///     RatFunc::<Bignum>::from_poly(Poly::from_coeffs(
///         cs.iter().map(|&c| Q::from_i128(c)).collect(),
///     ))
/// };
/// let window = Interval { lo: Q::from_i128(-2), hi: Q::from_i128(2) };
/// let tol = Q::new(1, 1024);
///
/// // Degenerate in µ̂ (`a ≡ b ≡ 0`) with `c(σ) = σ − 1/2`: the ruling is wholly inside the cutter
/// // while `c < 0`, wholly outside after, and the flip is at σ = 1/2.
/// let flip = MuCut { a: poly(&[]), b: poly(&[]), c: poly(&[-1, 2]) };
/// let got = coverage_events(&flip, &window, &tol).unwrap();
/// assert_eq!(got.len(), 1);
/// assert!(got[0].lo <= Q::new(1, 2) && Q::new(1, 2) <= got[0].hi);
///
/// // A wall that bounds µ̂ at all is not this class, however its `c` behaves.
/// let ordinary = MuCut { a: poly(&[]), b: poly(&[1]), c: poly(&[-1, 2]) };
/// assert!(coverage_events(&ordinary, &window, &tol).unwrap().is_empty());
/// ```
pub fn coverage_events<B: Backend>(
    form: &MuCut<B>,
    window: &Interval<B>,
    tol: &Rat<B>,
) -> Result<Vec<Interval<B>>, CutFitFault> {
    if !form.a.is_zero() || !form.b.is_zero() {
        return Ok(Vec::new());
    }
    let mut out = isolate_roots(&form.c, window, tol)?;
    out.sort_by(|a, b| a.lo.cmp(&b.lo));
    Ok(out)
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

// Hand-written so `B` need not be `Clone` (the backend markers are not).
impl<B: Backend> Clone for MuCut<B> {
    fn clone(&self) -> Self {
        MuCut {
            a: self.a.clone(),
            b: self.b.clone(),
            c: self.c.clone(),
        }
    }
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

/// The arc of a [`CutLoop`] running from one junction to another — the piece of a contour's
/// footprint that no graph `µ̂ = f(σ)` can carry, because it **turns around** a tangent ruling.
///
/// A loop from [`quadric_cut_loop`] traverses `left tangent → upper branch (σ ascending) → right
/// tangent → lower branch (σ descending) → close`, which is the same sense an outer boundary runs
/// (up the near cap, right along the top, down the far cap, left along the bottom). So the arc is a
/// **contiguous run** of the loop's pieces, and the only work is finding its two ends and trimming
/// them exactly.
///
/// The junctions are where the chains hand the boundary to the contour, which the run-corner
/// refinement already located; `from_upper`/`to_upper` say which branch each sits on, and that
/// alone determines how many tangents the arc wraps:
///
/// | `from_upper` | `to_upper` | tangents wrapped | when |
/// |---|---|---|---|
/// | upper | lower | **one** (the σ-max) | the contour takes over near the `σ_hi` end |
/// | lower | upper | **one** (the σ-min) | …near the `σ_lo` end |
/// | upper | upper | **two** | the contour bounds the whole *lower* side, so its two tangents are joined by one continuous run of contour boundary |
/// | lower | lower | **two** | the same, on the upper side |
///
/// Both boundary pieces are cut at the exact parameter where σ meets the junction — a piece is a
/// chord, so σ is linear in its parameter and the cut is one division in ℚ, leaving no sliver for a
/// micro-cap to paper over and nothing for the unroll's exact chaining check to reject.
///
/// `None` if the loop does not turn (fewer than two σ-extremes), or if a junction is not found on
/// the branch it was said to be on — both of which mean the caller's structure and this loop
/// disagree, which is a refusal rather than something to approximate.
pub fn tangent_turn_arc<B: Backend>(
    cut: &CutLoop<B>,
    from: &Rat<B>,
    from_upper: bool,
    to: &Rat<B>,
    to_upper: bool,
) -> Option<Vec<crate::pcurve::PCurve<B>>> {
    use core::cmp::Ordering;
    let n = cut.pieces.len();
    if n < 4 {
        return None;
    }
    // Each piece's start vertex, in traversal order.
    let start_of = |k: usize| cut.pieces[k].eval(&cut.pieces[k].domain.lo);
    let sig: Vec<Rat<B>> = (0..n)
        .map(|k| {
            let [s, _] = start_of(k)?;
            Some(s)
        })
        .collect::<Option<_>>()?;
    // The two turning indices: where σ stops rising and where it stops falling.
    let (mut at_max, mut at_min) = (0usize, 0usize);
    for k in 0..n {
        if sig[k].cmp(&sig[at_max]) == Ordering::Greater {
            at_max = k;
        }
        if sig[k].cmp(&sig[at_min]) == Ordering::Less {
            at_min = k;
        }
    }
    if at_max == at_min {
        return None;
    }
    // Walk the cycle in traversal order from the turn the `from` run **begins** at — the upper
    // (ascending) run starts at the σ-min turn, the lower (descending) one at the σ-max — so the
    // run we want is contiguous and forward from the first piece.
    let begin = if from_upper { at_min } else { at_max };
    // Twice round: a two-turn arc leaves its start run, crosses both extremes, and finishes on that
    // same run — which is only reachable on a second pass. One pass caps the walk at a single turn,
    // so the two-turn case can never complete and (correctly, but uselessly) returns `None`.
    let order: Vec<usize> = (0..2 * n).map(|i| (begin + i) % n).collect();
    // σ alone does not say which branch a junction is on — both branches cover the same σ. What
    // distinguishes them is **how many turns the walk has taken**, so `to` is matched only once the
    // arc has wrapped as many tangents as the two branch flags imply: one to cross to the other
    // branch, two to come back to the same one. (The first version guarded with an index count and
    // matched `to` on the approach, producing an "arc" that ran up to the tangent and stopped — a
    // graph, which is the one thing this function exists not to return.)
    let turns_needed = if from_upper == to_upper { 2 } else { 1 };

    let mut out: Vec<crate::pcurve::PCurve<B>> = Vec::new();
    let mut started = false;
    let mut turns = 0usize;
    for (i, &k) in order.iter().enumerate() {
        if i > 0 && (k == at_max || k == at_min) {
            turns += 1;
        }
        let piece = &cut.pieces[k];
        let [a, _] = piece.eval(&piece.domain.lo)?;
        let [b, _] = piece.eval(&piece.domain.hi)?;
        let spans = |s: &Rat<B>| {
            (a.cmp(s) != Ordering::Greater && s.cmp(&b) != Ordering::Greater)
                || (b.cmp(s) != Ordering::Greater && s.cmp(&a) != Ordering::Greater)
        };
        // A junction cut may land exactly on a piece end, leaving nothing of it: that is a clean
        // join, not a failure, so the degenerate remainder is dropped rather than kept whole.
        //
        // The parameter is solved **exactly**, not searched. `PCurve::params_at_sigma` bisects, and
        // a bisected parameter puts the arc's endpoint *near* the junction rather than on it — which
        // the unroll's chaining check compares over ℚ and rejects (`ArcDiscontinuity`), correctly.
        // A loop piece is a chord, so `σ(t) = a + (b − a)·t` and the junction is one division; a
        // piece whose σ is not affine is refused rather than approximated, since an inexact join
        // here is a boundary that does not close.
        let cut_at = |lo_side: bool, s: &Rat<B>| -> Option<Option<crate::pcurve::PCurve<B>>> {
            let sigma = &piece.sigma;
            if sigma.den().degree().unwrap_or(0) != 0 || sigma.num().degree().unwrap_or(0) > 1 {
                return None;
            }
            let nth = |p: &lattice::Poly<B>, i: usize| {
                p.coeffs()
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| Rat::from_i128(0))
            };
            let d0 = nth(sigma.den(), 0);
            if d0.sign() == 0 {
                return None;
            }
            let c0 = nth(sigma.num(), 0).div(&d0);
            let c1 = nth(sigma.num(), 1).div(&d0);
            if c1.sign() == 0 {
                return None;
            }
            let t = s.sub(&c0).div(&c1);
            let span = if lo_side {
                Interval {
                    lo: t,
                    hi: piece.domain.hi.clone(),
                }
            } else {
                Interval {
                    lo: piece.domain.lo.clone(),
                    hi: t,
                }
            };
            Some(piece.restrict(&span))
        };
        if !started {
            // `from` lies on the run the walk opens with, so it is matched before any turn.
            if turns > 0 || !spans(from) {
                continue;
            }
            if let Some(tail) = cut_at(true, from)? {
                out.push(tail);
            }
            started = true;
            continue;
        }
        if turns >= turns_needed && spans(to) {
            if let Some(head) = cut_at(false, to)? {
                out.push(head);
            }
            return if out.is_empty() { None } else { Some(out) };
        }
        out.push(piece.clone());
    }
    None
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
/// `docs/cutter-extrude-design.md` §2.1, read along one ruling. **Every** stretch the ruling spends
/// inside the solid, in increasing µ̂.
///
/// Every wall contributes its crossings; sorted, they cut the ruling into stretches, each wholly
/// inside the solid or wholly outside, so one membership test per stretch classifies it exactly.
/// This is the exact sibling of the resolver's float `extruded_shadow` — same construction and now
/// the same *shape* of answer (`Shadow(Vec<Patch>)` since AUTH.1e.1), but the crossings are
/// rationals rather than `f64`, because what is built from them is emitted geometry rather than a
/// structural decision. That the two views agree stretch-for-stretch is what §10.4's D2 contract
/// rests on.
///
/// An empty result means the ruling misses the solid. Two refusals survive the generalization,
/// because neither is about *how many* stretches there are: an inside stretch running to infinity is
/// [`ShadowUnbounded`](CutFitFault::ShadowUnbounded) — not an interior hole at all — and an
/// unreadable fill is [`ShadowUndecided`](CutFitFault::ShadowUndecided).
fn ruling_patches<B: Backend, F>(
    forms: &[MuCut<B>],
    sigma: &Rat<B>,
    inside: &F,
    sqrt_eps: &Rat<B>,
) -> Result<Vec<RulingPatch<B>>, CutFitFault>
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
            Ok(Vec::new())
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
    let mut found: Vec<RulingPatch<B>> = Vec::new();
    for pair in cuts.windows(2) {
        let (lo, hi) = (&pair[0], &pair[1]);
        let width = hi.0.sub(&lo.0);
        if width.sign() <= 0 {
            continue; // two walls crossing at the same µ̂ — a corner, not a stretch
        }
        if !decide(&lo.0.add(&hi.0).mul(&Rat::new(1, 2)), &width)? {
            continue;
        }
        // Merge with the previous stretch when they share their endpoint. A **carrier** is the
        // whole infinite line, not the profile edge on it, so a non-convex profile has carriers
        // that run through its own interior — the L's `y = 1` is the top of one arm and interior
        // to the other. A ruling crossing there gets a crossing point that is not a boundary
        // point, and reporting it would split one stretch into two abutting ones (measured: an L
        // reporting *three* stretches where a straight line can meet it in at most two). The
        // union of two intervals sharing an endpoint is the interval, so merging is exact rather
        // than a tolerance. Convex profiles never hit this — their carriers are supporting lines,
        // so every extra crossing lands outside the inside stretch — which is why AUTH.1e.4 could
        // not have seen it.
        match found.last_mut() {
            Some(prev) if prev.hi.cmp(&lo.0) == Ordering::Equal => {
                prev.hi = hi.0.clone();
                prev.hi_at = hi.1;
            }
            _ => found.push(RulingPatch {
                lo: lo.0.clone(),
                hi: hi.0.clone(),
                lo_at: lo.1,
                hi_at: hi.1,
            }),
        }
    }
    Ok(found)
}

/// Build **every** closed boundary loop of a cutter's footprint in the domain — the general
/// tracer, which lifts AUTH.1e.4's band restriction to any connected non-convex profile
/// (`docs/cutter-extrude-design.md` §11). It replaced that band builder outright: with the
/// footprint read stretch-by-stretch, a band is the one-stretch case and needs no code of its own.
///
/// The footprint is swept in σ. At each sampled ruling [`ruling_patches`] gives the stretches the
/// ruling spends inside the cutter, and between two consecutive rulings the stretches are matched
/// by µ̂-**overlap** — which reads off, with no case analysis of its own, whether a stretch
/// continues, is born, dies, merges with its neighbour or splits. Continuations become rail pieces;
/// births, deaths and merge saddles identify two boundary ends into one vertex, exactly as
/// [`quadric_cut_loop`] closes a band at its tangent rulings. The closed loops are then read off the
/// resulting graph, in which every boundary vertex has exactly two rails.
///
/// Sampling is driven by [`structure_events`]: the window is cut at the event brackets and each
/// resulting cell is sampled `√`-graded from both ends, since a branch turns like a square root at
/// a birth, a death or a saddle. **The events do not enter the matching**, which applies the same
/// rule to every consecutive pair of columns — so an event the sweep stepped over is a sampling
/// loss, not a wrong answer, and a profile corner is simply a cell boundary rather than the
/// dedicated bisection sweep §10.2 needed.
///
/// Certification is unchanged from §10.3 and is what the soundness rests on: every piece is bounded
/// against the wall its own endpoints name, **and** compared at its σ-midpoint against the boundary
/// the exact fill rule reports there, with the deviation folded into `eps`. `gap` — the half-width
/// closed at each pinch and saddle — is folded in too, so nothing is unaccounted.
///
/// Refuses rather than guessing: a footprint reaching the window edge is
/// [`ShadowUnbounded`](CutFitFault::ShadowUnbounded), a bracket in which stretches both merge and
/// split is [`ShadowEventTangled`](CutFitFault::ShadowEventTangled), and a boundary that does not
/// close is [`ShadowLoopOpen`](CutFitFault::ShadowLoopOpen).
#[allow(clippy::too_many_arguments)]
pub fn shadow_cut_loops<B: Backend, F>(
    chart: &Chart<B>,
    walls: &[CutSurface<B>],
    inside: F,
    window: &Interval<B>,
    w: &Rat<B>,
    segments: usize,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Verdict<Vec<CutLoop<B>>, CutFitFault, Rat<B>>
where
    F: Fn(&Rat<B>, &Rat<B>) -> Option<bool>,
{
    use core::cmp::Ordering;
    match shadow_loops_inner(chart, walls, &inside, window, w, segments, clearance, cfg) {
        Err(f) => Verdict::Refuted(f),
        Ok(loops) => {
            let drc = clearance.mul(&Rat::new(1, 2));
            let worst = loops
                .iter()
                .map(|l| l.eps.clone())
                .max_by(|a, b| a.cmp(b))
                .unwrap_or_else(|| Rat::from_i128(0));
            if loops.is_empty() {
                Verdict::Refuted(CutFitFault::DegenerateSpan)
            } else if worst.cmp(&drc) == Ordering::Less {
                Verdict::Verified(loops)
            } else {
                Verdict::Unresolved(worst)
            }
        }
    }
}

/// One boundary end of one stretch on one sampled ruling — the tracer's vertex before any
/// identification. `Lo`/`Hi` is which side of the stretch it bounds, and the side is kept because
/// the σ-midpoint honesty check compares against the boundary of the *same* side.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Lo,
    Hi,
}

/// Union-find with path halving, over the tracer's boundary ends.
fn uf_find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

/// [`shadow_cut_loops`]' body, in `Result` form so the refusals read as `?`.
#[allow(clippy::too_many_arguments)]
fn shadow_loops_inner<B: Backend, F>(
    chart: &Chart<B>,
    walls: &[CutSurface<B>],
    inside: &F,
    window: &Interval<B>,
    w: &Rat<B>,
    segments: usize,
    clearance: &Rat<B>,
    cfg: &DevConfig<B>,
) -> Result<Vec<CutLoop<B>>, CutFitFault>
where
    F: Fn(&Rat<B>, &Rat<B>) -> Option<bool>,
{
    use crate::pcurve::snap;
    use core::cmp::Ordering;
    /// The dyadic grid every emitted coordinate is snapped to (as [`quadric_cut_loop`]).
    const BITS: u32 = 30;
    /// Sub-intervals per piece for the per-piece certificate.
    const PIECE_SUBDIV: usize = 64;

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
    let unit = Rat::new(1, 1i128 << BITS);
    let half_of = |a: &Rat<B>, b: &Rat<B>| a.add(b).mul(&Rat::new(1, 2));

    // — 1. Localize the footprint inside the (possibly much larger) window, exactly as the band
    //   builder does: `window` may be a bounding circle's, and a budget spread over *that* buys no
    //   resolution where the cutter actually is. A footprint reaching the scan's own first or last
    //   ruling has no closing boundary there — §10.2's `ShadowUnbounded`, and the reason the two
    //   flanking empty columns below are load-bearing rather than decorative. —
    let n = segments.max(2);
    let scan = (4 * n).max(48);
    let width = window.hi.sub(&window.lo);
    let at = |k: usize| {
        window
            .lo
            .add(&width.mul(&Rat::new(k as i128, scan as i128)))
    };
    let (mut first, mut last) = (None, None);
    for k in 0..=scan {
        if !ruling_patches(&forms, &at(k), inside, &cfg.sqrt_eps)?.is_empty() {
            first.get_or_insert(k);
            last = Some(k);
        }
    }
    let (first, last) = match (first, last) {
        (Some(f), Some(l)) => (f, l),
        _ => return Err(CutFitFault::DegenerateSpan),
    };
    if first == 0 || last == scan {
        return Err(CutFitFault::ShadowUnbounded);
    }
    let span = Interval {
        lo: at(first - 1),
        hi: at(last + 1),
    };

    // — 2. Columns: the footprint's own σ-range cut at the event brackets, each cell √-graded from
    //   both ends (a branch turns like a square root at a birth, a death or a saddle). —
    let events = structure_events(&forms, &span, &unit)?;
    let mut cells: Vec<(Rat<B>, Rat<B>)> = Vec::new();
    let mut cur = span.lo.clone();
    for e in &events {
        if cur.cmp(&e.at.lo) == Ordering::Less {
            cells.push((cur.clone(), e.at.lo.clone()));
        }
        if e.at.hi.cmp(&cur) == Ordering::Greater {
            cur = e.at.hi.clone();
        }
    }
    if cur.cmp(&span.hi) == Ordering::Less {
        cells.push((cur, span.hi.clone()));
    }
    // `segments` is a budget for the whole footprint, not per cell: an L has a cell per corner, and
    // spending `n` on each of them buys resolution nobody asked for and pays for it in emitted
    // pieces — which become faces downstream. Cells share the budget by width, with a floor of two
    // so even a sliver cell keeps its own two ends.
    let total = cells
        .iter()
        .fold(Rat::from_i128(0), |acc, (a, b)| acc.add(&b.sub(a)));
    let mut sigmas: Vec<Rat<B>> = Vec::new();
    for (a, b) in &cells {
        let width = b.sub(a);
        // `floor` lands on an integer-valued rational; step up from 2 until it is reached, which
        // needs no exact→integer conversion (there is none) and is bounded by `n`.
        let mut share: i128 = 2;
        if total.sign() > 0 {
            let want = width.div(&total).mul(&Rat::from_i128(n as i128));
            while share < n as i128 && Rat::from_i128(share + 1).cmp(&want) != Ordering::Greater {
                share += 1;
            }
        }
        let half = width.mul(&Rat::new(1, 2));
        sigmas.push(snap(a, BITS));
        sigmas.push(snap(b, BITS));
        // One grid step inside each end — §10.2's corner bracketing, now at every cell boundary.
        // This is what keeps a birth, a death or a saddle *tight*: the two ends identified there
        // are a grid step from the true event, so the half-width folded into `eps` is the branch's
        // own width at 2⁻³⁰ rather than at whatever the interior grading happened to reach. Without
        // it a polygon's pinch was measured 12% of a cell inside, and `gap` — not the certificate —
        // dominated ε.
        let (lo_in, hi_in) = (a.add(&unit), b.sub(&unit));
        if lo_in.cmp(b) == Ordering::Less {
            sigmas.push(snap(&lo_in, BITS));
        }
        if hi_in.cmp(a) == Ordering::Greater {
            sigmas.push(snap(&hi_in, BITS));
        }
        for k in 1..share {
            let f = Rat::new(k, share);
            let d = f.mul(&f).mul(&half);
            sigmas.push(snap(&a.add(&d), BITS));
            sigmas.push(snap(&b.sub(&d), BITS));
        }
    }
    // The two flanking columns are the scan's own samples, verbatim and unsnapped: their emptiness
    // is what was *verified* above, and snapping moves a column onto the grid — off the sample and,
    // at a footprint that starts within a grid step of it, into the material. That reads as a
    // footprint running off the span's edge and refuses a perfectly good cut. They contribute no
    // vertices (being empty), so nothing downstream sees their non-dyadic σ.
    sigmas.push(span.lo.clone());
    sigmas.push(span.hi.clone());
    sigmas.retain(|s| s.cmp(&span.lo) != Ordering::Less && s.cmp(&span.hi) != Ordering::Greater);
    sigmas.sort();
    sigmas.sort();
    sigmas.dedup_by(|a, b| (*a).cmp(&*b) == Ordering::Equal);
    if sigmas.len() < 3 {
        return Err(CutFitFault::DegenerateSpan);
    }

    // — 3. Sample every column. The two flanking columns are empty by the scan above, so every
    //   boundary closes inside the span rather than running off its edge. —
    let mut cols: Vec<(Rat<B>, Vec<RulingPatch<B>>)> = Vec::with_capacity(sigmas.len());
    for s in sigmas {
        let ps = ruling_patches(&forms, &s, inside, &cfg.sqrt_eps)?;
        cols.push((s, ps));
    }
    if !cols[0].1.is_empty() || !cols[cols.len() - 1].1.is_empty() {
        return Err(CutFitFault::ShadowUnbounded);
    }

    // — 4. Boundary ends, and the identifications that close them. —
    let mut base: Vec<usize> = Vec::with_capacity(cols.len());
    let mut pos: Vec<(Rat<B>, Rat<B>)> = Vec::new();
    let mut label: Vec<Option<WallRoot>> = Vec::new();
    for (s, ps) in &cols {
        base.push(pos.len());
        for p in ps {
            pos.push((s.clone(), snap(&p.lo, BITS)));
            label.push(Some(p.lo_at));
            pos.push((s.clone(), snap(&p.hi, BITS)));
            label.push(Some(p.hi_at));
        }
    }
    let end = |ci: usize, j: usize, side: Side| -> usize {
        base[ci] + 2 * j + usize::from(side == Side::Hi)
    };
    let mut parent: Vec<usize> = (0..pos.len()).collect();
    let mut edges: Vec<(usize, usize, Side)> = Vec::new();
    // The largest half-width closed at a pinch or a saddle — the generalization of `tangent_gap`,
    // and folded into `eps` the same way.
    let mut gap = Rat::from_i128(0);
    let join = |parent: &mut Vec<usize>,
                pos: &mut Vec<(Rat<B>, Rat<B>)>,
                label: &mut Vec<Option<WallRoot>>,
                gap: &mut Rat<B>,
                a: usize,
                b: usize| {
        let (ra, rb) = (uf_find(parent, a), uf_find(parent, b));
        if ra == rb {
            return;
        }
        let mid = half_of(&pos[ra].1, &pos[rb].1);
        let h = abs_rat(&pos[ra].1.sub(&pos[rb].1)).mul(&Rat::new(1, 2));
        if h.cmp(gap) == Ordering::Greater {
            *gap = h;
        }
        parent[rb] = ra;
        pos[ra] = (pos[ra].0.clone(), snap(&mid, BITS));
        // A vertex two boundaries meet at lies on neither wall alone, so it names none — the same
        // convention the band's two pinch vertices use.
        label[ra] = None;
    };

    for ci in 0..cols.len() - 1 {
        let (l, r) = (&cols[ci].1, &cols[ci + 1].1);
        // Same count ⇒ match by µ̂ order. Stretches are ordered and disjoint and cannot cross, so
        // index matching is the topologically consistent reading, and it is the *only* reliable one
        // where two columns are far apart: overlap alone ties a thin lobe to its neighbour as soon
        // as the lobe travels further than its own width between samples, which reads as a merge
        // and a split at once (`ShadowEventTangled`) on perfectly ordinary geometry. The overlap
        // walk is for the columns that straddle an event, where the count changes and the two
        // columns sit a bracket apart.
        if l.len() == r.len() {
            for k in 0..l.len() {
                edges.push((end(ci, k, Side::Lo), end(ci + 1, k, Side::Lo), Side::Lo));
                edges.push((end(ci, k, Side::Hi), end(ci + 1, k, Side::Hi), Side::Hi));
            }
            continue;
        }
        let overlaps = |a: &RulingPatch<B>, b: &RulingPatch<B>| {
            a.lo.cmp(&b.hi) != Ordering::Greater && b.lo.cmp(&a.hi) != Ordering::Greater
        };
        let (mut i, mut j) = (0usize, 0usize);
        while i < l.len() || j < r.len() {
            if i < l.len() && j < r.len() && overlaps(&l[i], &r[j]) {
                // Grow the overlap component: consecutive stretches on either side that reach it.
                let (i0, j0) = (i, j);
                let (mut ie, mut je) = (i + 1, j + 1);
                loop {
                    let mut grew = false;
                    while ie < l.len() && (j0..je).any(|jj| overlaps(&l[ie], &r[jj])) {
                        ie += 1;
                        grew = true;
                    }
                    while je < r.len() && (i0..ie).any(|ii| overlaps(&l[ii], &r[je])) {
                        je += 1;
                        grew = true;
                    }
                    if !grew {
                        break;
                    }
                }
                let (p, q) = (ie - i0, je - j0);
                match (p, q) {
                    (1, 1) => {
                        edges.push((end(ci, i0, Side::Lo), end(ci + 1, j0, Side::Lo), Side::Lo));
                        edges.push((end(ci, i0, Side::Hi), end(ci + 1, j0, Side::Hi), Side::Hi));
                    }
                    // A merge: the group's outer rails continue, and each inner pair of ends is a
                    // saddle — the two branches that met there.
                    (_, 1) => {
                        edges.push((end(ci, i0, Side::Lo), end(ci + 1, j0, Side::Lo), Side::Lo));
                        edges.push((
                            end(ci, ie - 1, Side::Hi),
                            end(ci + 1, j0, Side::Hi),
                            Side::Hi,
                        ));
                        for k in i0..ie - 1 {
                            join(
                                &mut parent,
                                &mut pos,
                                &mut label,
                                &mut gap,
                                end(ci, k, Side::Hi),
                                end(ci, k + 1, Side::Lo),
                            );
                        }
                    }
                    // A split: the mirror image, with the saddles on the right column.
                    (1, _) => {
                        edges.push((end(ci, i0, Side::Lo), end(ci + 1, j0, Side::Lo), Side::Lo));
                        edges.push((
                            end(ci, i0, Side::Hi),
                            end(ci + 1, je - 1, Side::Hi),
                            Side::Hi,
                        ));
                        for k in j0..je - 1 {
                            join(
                                &mut parent,
                                &mut pos,
                                &mut label,
                                &mut gap,
                                end(ci + 1, k, Side::Hi),
                                end(ci + 1, k + 1, Side::Lo),
                            );
                        }
                    }
                    _ => return Err(CutFitFault::ShadowEventTangled),
                }
                i = ie;
                j = je;
            } else if i < l.len() && (j >= r.len() || l[i].hi.cmp(&r[j].lo) == Ordering::Less) {
                // A death: the stretch's two ends close on one vertex, as a band does at a tangent.
                join(
                    &mut parent,
                    &mut pos,
                    &mut label,
                    &mut gap,
                    end(ci, i, Side::Lo),
                    end(ci, i, Side::Hi),
                );
                i += 1;
            } else {
                // A birth: the same, on the right column.
                join(
                    &mut parent,
                    &mut pos,
                    &mut label,
                    &mut gap,
                    end(ci + 1, j, Side::Lo),
                    end(ci + 1, j, Side::Hi),
                );
                j += 1;
            }
        }
    }

    // — 5. Read the closed loops off the graph: every boundary vertex carries exactly two rails. —
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); pos.len()];
    let mut ends: Vec<(usize, usize)> = Vec::with_capacity(edges.len());
    for (ei, (a, b, _)) in edges.iter().enumerate() {
        let (ra, rb) = (uf_find(&mut parent, *a), uf_find(&mut parent, *b));
        adj[ra].push(ei);
        adj[rb].push(ei);
        ends.push((ra, rb));
    }
    for (v, es) in adj.iter().enumerate() {
        if !es.is_empty() && es.len() != 2 && uf_find(&mut parent, v) == v {
            return Err(CutFitFault::ShadowLoopOpen);
        }
    }
    let mut used = vec![false; edges.len()];
    let mut cycles: Vec<Vec<usize>> = Vec::new();
    for e0 in 0..edges.len() {
        if used[e0] {
            continue;
        }
        let start = ends[e0].0;
        let mut verts = vec![start];
        let (mut v, mut e) = (start, e0);
        loop {
            used[e] = true;
            let (a, b) = ends[e];
            v = if a == v { b } else { a };
            if v == start {
                break;
            }
            verts.push(v);
            match adj[v].iter().find(|&&ei| !used[ei]) {
                Some(&ei) => e = ei,
                None => return Err(CutFitFault::ShadowLoopOpen),
            }
        }
        if verts.len() >= 3 {
            cycles.push(verts);
        }
    }
    if cycles.is_empty() {
        return Err(CutFitFault::DegenerateSpan);
    }

    // — 6. Nested loops are the ring, and the ring is a different feature (§11.6): a hole with a
    //   hole of its own leaves an island of material floating free, which is two parts rather than
    //   one cut. Refuse by name rather than emit a loop the downstream would have to interpret.
    //
    //   Containment is decided by an exact even-odd ray cast in the domain — the loops are
    //   polylines over ℚ, so this needs no tolerance; the half-open span rule (`lo ≤ σ < hi`)
    //   is what keeps a ray through a vertex from being counted twice. —
    let contains = |c: &[usize], p: &(Rat<B>, Rat<B>)| -> bool {
        let mut odd = false;
        for k in 0..c.len() {
            let (a, b) = (&pos[c[k]], &pos[c[(k + 1) % c.len()]]);
            let (lo, hi, mlo, mhi) = if a.0.cmp(&b.0) == Ordering::Less {
                (&a.0, &b.0, &a.1, &b.1)
            } else {
                (&b.0, &a.0, &b.1, &a.1)
            };
            if lo.cmp(&p.0) != Ordering::Greater && p.0.cmp(hi) == Ordering::Less {
                // The edge's µ̂ at this σ, above the point ⇒ one crossing of the upward ray.
                let t = p.0.sub(lo).div(&hi.sub(lo));
                if mlo.add(&mhi.sub(mlo).mul(&t)).cmp(&p.1) == Ordering::Greater {
                    odd = !odd;
                }
            }
        }
        odd
    };
    for a in 0..cycles.len() {
        for b in 0..cycles.len() {
            if a != b && contains(&cycles[b], &pos[cycles[a][0]]) {
                return Err(CutFitFault::ShadowNested);
            }
        }
    }

    // — 7. Emit and certify, piece by piece, exactly as the band does (§10.3). —
    let mut out = Vec::with_capacity(cycles.len());
    for verts in cycles {
        let mut eps = gap.clone();
        let mut pieces = Vec::with_capacity(verts.len());
        for k in 0..verts.len() {
            let (va, vb) = (verts[k], verts[(k + 1) % verts.len()]);
            let (a, b) = (pos[va].clone(), pos[vb].clone());
            if a.0.cmp(&b.0) == Ordering::Equal {
                // Consecutive vertices share a σ only if a rail was dropped; an open or doubled
                // loop is worse downstream than a refusal here.
                return Err(CutFitFault::ShadowLoopOpen);
            }
            let piece = segment(&a, &b);
            let mut targets: Vec<usize> = Vec::with_capacity(2);
            for v in [va, vb] {
                if let Some((wi, _)) = label[v]
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
            // On the boundary, not merely near a wall: compare the emitted chord at its own
            // σ-midpoint against the nearest boundary the fill rule reports there. A missed event
            // lands here as a loose ε, never as a hole that is quietly the wrong shape.
            let sm = half_of(&a.0, &b.0);
            let mm = half_of(&a.1, &b.1);
            let truth = ruling_patches(&forms, &sm, inside, &cfg.sqrt_eps)?;
            let mut best: Option<Rat<B>> = None;
            for p in &truth {
                for t in [&p.lo, &p.hi] {
                    let d = abs_rat(&mm.sub(t));
                    if best.as_ref().is_none_or(|m| d.cmp(m) == Ordering::Less) {
                        best = Some(d);
                    }
                }
            }
            if let Some(d) = best
                && d.cmp(&eps) == Ordering::Greater
            {
                eps = d;
            }
            pieces.push(piece);
        }
        out.push(CutLoop {
            pieces,
            eps: crate::pcurve::snap_up(&eps, BITS),
            tangent_gap: gap.clone(),
        });
    }
    Ok(out)
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
    // Recognition is a property of the surface, so it happens once and not per sub-interval.
    let rev = match surface {
        CutSurface::Quadric(q) => RevCone::recognize(q),
        _ => None,
    };
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
        let dist = match surface_distance_on(surface, rev.as_ref(), &x, &half, cfg) {
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
    // Round each component (DEV.2a). `eval_ratfunc_on` already hands back ~18-digit enclosures,
    // but rational **addition multiplies denominators**, so the five ops below chained them to
    // ~120 digits — on every one of the thousands of sub-interval evaluations a cut certificate
    // makes. Outward rounding costs `2^-60 ≈ 8.7e-19` per op, fifteen orders below any ε that
    // matters here, and keeps containment by construction.
    Some([
        p[0].add(&mu.mul(&r[0])).add(&wv.mul(&n[0])).rounded(),
        p[1].add(&mu.mul(&r[1])).add(&wv.mul(&n[1])).rounded(),
        p[2].add(&mu.mul(&r[2])).add(&wv.mul(&n[2])).rounded(),
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
    rev: Option<&RevCone<B>>,
    x: &[RatIv<B>; 3],
    radius: &Rat<B>,
    cfg: &DevConfig<B>,
) -> DistOn<B> {
    let closed_form = |d: Option<Rat<B>>| match d {
        Some(d) => DistOn::Bound(d),
        None => DistOn::Fault(CutFitFault::DegenerateSurface),
    };
    match (surface, rev) {
        // Recognized as a cone of revolution: the closed-form distance, on the box the caller
        // enclosed. Still a box — a p-curve is parametrized over its own `t`, so there is no
        // symbolic residual in σ to be had — but the bound is the geometric distance itself rather
        // than a first-order estimate inside an inflated ball, and no ball means no apex clearance
        // to trip over.
        (CutSurface::Quadric(_), Some(cone)) => {
            let a2 = dot3(&cone.axis, &cone.axis);
            let v: [RatIv<B>; 3] =
                core::array::from_fn(|i| x[i].sub(&RatIv::point(cone.apex[i].clone())));
            let sum = |f: &dyn Fn(usize) -> RatIv<B>| f(0).add(&f(1)).add(&f(2));
            let n2 = sum(&|i| v[i].mul(&v[i]));
            let ta = sum(&|i| v[i].mul(&RatIv::point(cone.axis[i].clone())));
            let s2 = n2.sub(&ta.mul(&ta).mul(&RatIv::point(a2.recip())));
            DistOn::Bound(cone.dist_hi(&n2, &ta, &s2, &cone.sin_scaled(cfg), cfg))
        }
        (CutSurface::Quadric(q), None) => {
            quadric_distance_on(&q.m, &q.b, &q.c, &q.nappe, x, radius, cfg)
        }
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
                .sub(&RatIv::point(d.clone()))
                .rounded();
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
            // Rounded at each step (DEV.2a): squaring and summing three enclosures, then taking a
            // root, is where the digits ran away — an exact rational √-enclosure must *narrow* as
            // its input box narrows, so finer subdivision bought hundreds of digits rather than a
            // tighter answer. Bounded here instead, outward, so containment is preserved.
            let d2 = dv[0]
                .mul(&dv[0])
                .add(&dv[1].mul(&dv[1]))
                .add(&dv[2].mul(&dv[2]))
                .rounded();
            let ad = dot(&dv, axis_dir).rounded();
            let perp2 = d2
                .sub(&ad.mul(&ad).mul(&RatIv::point(a2.recip())))
                .rounded();
            let rho = sqrt_on(&perp2, &cfg.sqrt_eps).rounded();
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

    // The nappe condition, at **the ball this bound actually uses**.
    //
    // The lemma places a zero of `F` within `e ≤ r` of the traced point, so the statement that has
    // to hold is "the ball of radius `r` lies on the authored nappe's side" — no larger. It used to
    // be checked once at the full working radius `clearance/2`, which bundled a DRC cushion into a
    // soundness gate, and the cushion is what refused real geometry: measured on the device's
    // imported bore, the selector clears at `r/2` and at every smaller ball and fails **only** at
    // the constant. Both §4.1 conditions still bite — a trace that genuinely reaches the mirror
    // nappe, or the apex on the selector's own boundary plane, fails at every radius including the
    // smallest, and that is still [`CutFitFault::NappeCrossed`].
    let nappe_ok = |r: &Rat<B>| {
        dot_iv(&nappe.n, &inflate(r))
            .sub(&RatIv::point(nappe.d.clone()))
            .lo()
            .sign()
            > 0
    };

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
    let mut on_nappe = false;
    for k in (0..=STEPS).rev() {
        let r = radius.mul(&Rat::new(1, 1i128 << k));
        // A ball that reaches the mirror nappe cannot carry the claim, whatever it bounds. The
        // sequence grows, so once one fails so does every later one — but `continue` rather than
        // `break` keeps the loop's shape independent of that.
        if !nappe_ok(&r) {
            continue;
        }
        on_nappe = true;
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
    // Not even the smallest ball stays on the authored nappe: the trace reaches the mirror sheet or
    // the apex, which no amount of refinement fixes.
    if !on_nappe {
        return DistOn::Fault(CutFitFault::NappeCrossed);
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

    // A quadric that *is* a cylinder of revolution is measured as one. Recognition sits here, where
    // the certificate is computed, rather than where the wall is built — so a hand-authored
    // `Cylinder` and a swept profile circle take the same arm without the builders having to agree
    // on a normal form. [`RevCone`] does the same one arm down; between them they cover both apex
    // kinds a sketch can be swept from.
    let recognized = match surface {
        CutSurface::Quadric(q) => RevCylinder::recognize(q).map(|c| c.as_cut_surface()),
        _ => None,
    };
    let surface = recognized.as_ref().unwrap_or(surface);

    let eps = match surface {
        // A cone of revolution *does* have a closed-form distance, so it joins the two arms below
        // rather than paying the general quadric's box price — same certificate, one recognition
        // step ([`RevCone`]) ahead of it. A general quadric has no such parametrization: its
        // first-order bound works on a 3-D ball around the traced point, so the point itself has to
        // be enclosed first, and the lost σ↔µ̂ cancellation shows up as needing far more `subdiv`
        // for the same ε — not as a weaker claim.
        CutSurface::Quadric(q) => match RevCone::recognize(q) {
            Some(cone) => {
                let v = traced.sub(&const_vec3(&cone.apex)); // X − p
                let ax = const_vec3(&cone.axis);
                let a2 = dot3(&cone.axis, &cone.axis);
                let n2 = v.dot(&v).reduce(); // |X − p|²
                let ta = v.dot(&ax).reduce(); // (X − p)·a
                // s2 = |v|² − (v·a)²/|a|², the squared distance to the axis.
                let s2 = n2.sub(&ta.mul(&ta).scale(&a2.recip())).reduce();
                let sin_scaled = cone.sin_scaled(cfg);
                let mut eps = Rat::from_i128(0);
                for k in 0..n_sub {
                    let sig = subiv(lo, &width, k);
                    let (Some(n2v), Some(tav), Some(s2v)) = (
                        eval_ratfunc_on(&n2, &sig),
                        eval_ratfunc_on(&ta, &sig),
                        eval_ratfunc_on(&s2, &sig),
                    ) else {
                        return Verdict::Refuted(CutFitFault::PoleInEval);
                    };
                    eps = max_rat(eps, cone.dist_hi(&n2v, &tav, &s2v, &sin_scaled, cfg));
                }
                eps
            }
            None => {
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
        },
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

    /// The `(3, 4, 5)` cone: apex at the origin, axis `+z`, `tan α = 3/4` — so `cos²α = 16/25` and
    /// every quantity the recognizer must recover is exactly rational.
    fn cone345() -> Quadric<Bignum> {
        let q = Q::from_i128;
        let wall = crate::extrude::ellipse_wall(
            &[q(0), q(0), q(4)],                              // the circle of radius 3 …
            &[q(3), q(0), q(0)],                              // … as the unit circle of this frame,
            &[q(0), q(3), q(0)],                              // … at height 4,
            &crate::extrude::Apex::point([q(0), q(0), q(0)]), // cast from the origin.
        )
        .expect("a real cone");
        match wall {
            CutSurface::Quadric(b) => *b,
            _ => panic!("a circle cast from a point sweeps a quadric"),
        }
    }

    /// An upper bound on the distance from an exact point to a recognized cone's authored nappe.
    fn cone_dist(cone: &RevCone<Bignum>, x: [Q; 3], cfg: &DevConfig<Bignum>) -> Q {
        let v: [Q; 3] = core::array::from_fn(|i| x[i].sub(&cone.apex[i]));
        let a2 = dot3(&cone.axis, &cone.axis);
        let n2 = dot3(&v, &v);
        let ta = dot3(&v, &cone.axis);
        let s2 = n2.sub(&ta.mul(&ta).div(&a2));
        cone.dist_hi(
            &RatIv::point(n2),
            &RatIv::point(ta),
            &RatIv::point(s2),
            &cone.sin_scaled(cfg),
            cfg,
        )
    }

    /// **A cone of revolution is recovered from its coefficients exactly, and nothing else is.**
    ///
    /// The recognizer is what moves a normal cut off the general quadric's box bound, so what it
    /// accepts has to be right in both directions: it must find the apex, axis and half-angle of a
    /// circular cone as exact rationals, and it must decline every quadric that only looks like one.
    /// The three negatives are the three ways the extraction can fail — an oblique cast gives an
    /// *elliptic* cone (the rank-one step fails), a cast from infinity gives a cylinder (the double
    /// eigenvalue is zero), and shifting the constant gives a hyperboloid of one sheet, which has
    /// the same `M` and `b` as the cone it is asymptotic to and differs only in the last check.
    ///
    /// The cylinder declines at `cos²α = 1`, not at the double root — its eigenvalues are `λ, λ, 0`,
    /// so the closed form returns `λ`, and what gives it away is the *half-angle* going to zero with
    /// the apex at infinity. [`RevCylinder`] picks it up from there.
    #[test]
    fn a_cone_of_revolution_is_recognized_exactly_and_its_look_alikes_are_not() {
        let q = Q::from_i128;
        let cone = RevCone::recognize(&cone345()).expect("a right circular cone");
        assert!(cone.cos2.sub(&Q::new(16, 25)).is_zero(), "cos²α = (4/5)²");
        for i in 0..3 {
            assert!(cone.apex[i].is_zero(), "the apex is the cast point");
        }
        // The axis is `+z` up to a positive scale, and points into the authored nappe (`z > 0`).
        assert!(cone.axis[0].is_zero() && cone.axis[1].is_zero());
        assert!(cone.axis[2].sign() > 0, "oriented into the authored nappe");

        let quadric_of = |wall: CutSurface<Bignum>| match wall {
            CutSurface::Quadric(b) => *b,
            _ => panic!("expected a quadric wall"),
        };
        let oblique = quadric_of(
            crate::extrude::ellipse_wall(
                &[q(0), q(0), q(4)],
                &[q(3), q(0), q(0)],
                &[q(0), q(3), q(0)],
                &crate::extrude::Apex::point([q(1), q(0), q(0)]), // off the circle's axis
            )
            .expect("a real cone"),
        );
        assert!(
            RevCone::recognize(&oblique).is_none(),
            "an oblique cast is elliptic, not a cone of revolution"
        );
        let cylinder = quadric_of(
            crate::extrude::ellipse_wall(
                &[q(0), q(0), q(4)],
                &[q(3), q(0), q(0)],
                &[q(0), q(3), q(0)],
                &crate::extrude::Apex::direction([q(0), q(0), q(1)]).expect("a direction"),
            )
            .expect("a real cylinder"),
        );
        assert!(
            RevCone::recognize(&cylinder).is_none(),
            "a cast from infinity has no apex to find"
        );
        let mut hyperboloid = cone345();
        hyperboloid.c = hyperboloid.c.sub(&q(1));
        assert!(
            RevCone::recognize(&hyperboloid).is_none(),
            "the same asymptotic cone, and not a cone"
        );
    }

    /// **A cylinder of revolution is recovered too, and it is the sweep an imported outline uses.**
    ///
    /// A plan-view drawing has to be swept *straight down* to land where it was drawn, so the
    /// straight drill is not an exotic apex kind — it is the default for a cut file. Its circle
    /// clears to a quadric all the same, and without this the certificate falls to the box bound:
    /// measured **ε 5.56 against 1.53** for the same Ø 8 bore on the acceptance device, the whole
    /// difference being which arm ran.
    ///
    /// The negatives are the three ways the extraction can fail, and each fails at a different
    /// step: a *cone* has no zero eigenvalue at all; a **half**-cylinder (a live nappe selector) is
    /// declined outright, because the distance to a whole cylinder is a *lower* bound for the
    /// distance to half of one and a certificate may not be optimistic; and an **elliptic**
    /// cylinder — a circle in a frame whose axes are not orthonormal — fails the rank-one identity.
    #[test]
    fn a_cylinder_of_revolution_is_recognized_exactly_and_carries_the_closed_form() {
        let q = Q::from_i128;
        let quadric_of = |wall: CutSurface<Bignum>| match wall {
            CutSurface::Quadric(b) => *b,
            _ => panic!("expected a quadric wall"),
        };
        let straight = quadric_of(
            crate::extrude::ellipse_wall(
                &[q(0), q(0), q(4)],
                &[q(3), q(0), q(0)],
                &[q(0), q(3), q(0)],
                &crate::extrude::Apex::direction([q(0), q(0), q(1)]).expect("a direction"),
            )
            .expect("a real cylinder"),
        );
        let cyl = RevCylinder::recognize(&straight).expect("a cylinder of revolution");
        assert!(cyl.r2.sub(&q(9)).is_zero(), "r² = 3²");
        assert!(
            cyl.axis_dir[0].is_zero() && cyl.axis_dir[1].is_zero() && !cyl.axis_dir[2].is_zero(),
            "the axis is ±z"
        );
        // Any point of the axis will do, and this one is on the plane through the origin normal to
        // it — but what matters is that it *is* on the axis, which the residual is the test of.
        assert!(cyl.axis_point[0].is_zero() && cyl.axis_point[1].is_zero());

        // The recognized surface is the same point set: on it, off it, and by how much.
        let surface = cyl.as_cut_surface();
        for (x, y, z, want) in [(3, 0, 0, 0), (0, 3, 7, 0), (5, 0, 0, 16), (0, 0, 2, -9)] {
            let p = [q(x), q(y), q(z)];
            assert_eq!(
                surface.residual(&p).expect("a real axis"),
                q(want),
                "at ({x}, {y}, {z})"
            );
            assert_eq!(straight.nappe.n.iter().filter(|c| !c.is_zero()).count(), 0);
        }

        assert!(
            RevCylinder::recognize(&cone345()).is_none(),
            "a cone has no zero eigenvalue"
        );
        let mut half = straight.clone();
        half.nappe = Nappe {
            n: [q(0), q(0), q(1)],
            d: q(0),
        };
        assert!(
            RevCylinder::recognize(&half).is_none(),
            "half a cylinder is not a cylinder — the whole one's distance would be optimistic"
        );
        let elliptic = quadric_of(
            crate::extrude::ellipse_wall(
                &[q(0), q(0), q(4)],
                &[q(3), q(0), q(0)],
                &[q(0), q(5), q(0)], // a longer v axis: the profile circle is an ellipse
                &crate::extrude::Apex::direction([q(0), q(0), q(1)]).expect("a direction"),
            )
            .expect("a real cylinder"),
        );
        assert!(
            RevCylinder::recognize(&elliptic).is_none(),
            "an elliptic cylinder has no single radius"
        );
    }

    /// **The recognized cone's bound is the geometric distance, on both sides of the apex.**
    ///
    /// On the `(3, 4, 5)` cone the two witnesses are exact: `(0, 0, 5)` on the axis drops
    /// perpendicular to the generatrix at `5·sin α = 3`, and its mirror `(0, 0, −5)` is behind the
    /// apex, where the nearest point of the authored nappe *is* the apex, at `5`. The second is the
    /// case the general quadric arm cannot express at all — it bounds the distance to `{F = 0}`,
    /// which includes the mirror nappe, and so has to refuse rather than answer.
    #[test]
    fn the_cone_distance_is_the_geometric_one_on_either_side_of_the_apex() {
        let q = Q::from_i128;
        let cfg = DevConfig {
            terms: 14,
            sqrt_eps: Q::new(1, 1_000_000_000),
        };
        let cone = RevCone::recognize(&cone345()).expect("a right circular cone");
        let tol = Q::new(1, 1_000_000);
        let close = |got: Q, want: i128| {
            let d = got.sub(&q(want));
            assert!(
                d.cmp(&tol) == core::cmp::Ordering::Less
                    && d.cmp(&tol.neg()) == core::cmp::Ordering::Greater,
                "expected {want}, got {}",
                to_f64(&got)
            );
        };
        close(cone_dist(&cone, [q(3), q(0), q(4)], &cfg), 0); // on the cone
        close(cone_dist(&cone, [q(0), q(0), q(5)], &cfg), 3); // 5·sin α
        close(cone_dist(&cone, [q(0), q(0), q(-5)], &cfg), 5); // behind the apex
    }

    /// **The arc turns around the tangent, which is precisely what a graph cannot do.**
    ///
    /// A vertical cylinder's footprint on the device cone, as the closed loop
    /// [`quadric_cut_loop`] traces; the arc cut out of it between two junction σ must
    ///
    /// 1. **reverse in σ** — its σ rises to the tangent and falls back, so no `µ̂ = f(σ)` covers it;
    /// 2. **start and end exactly at the junctions**, not at whatever sample node happened to be
    ///    nearest, since a chain joins onto it there and a sliver would need a micro-cap;
    /// 3. **reach past both junctions** in σ, i.e. contain the tangent rather than stopping short.
    ///
    /// (1) is the load-bearing one: a run of pieces that merely *approached* the tangent would
    /// satisfy the endpoint checks and still be a graph.
    #[test]
    fn a_turn_arc_reverses_in_sigma_and_starts_where_it_is_told() {
        use crate::cone::DevConfig;
        let chart = cone();
        let zero = Q::from_i128(0);
        let surface = CutSurface::Cylinder {
            axis_point: [zero.clone(), Q::new(11, 5), zero.clone()],
            axis_dir: [zero.clone(), zero.clone(), Q::from_i128(1)],
            r2: Q::new(1, 25),
        };
        let form = cut_mu_form(&chart, &surface, &zero).expect("a pullback");
        let br = tangent_events(&form, &ivl(-1, 1), &Q::new(1, 1 << 40)).expect("isolable");
        assert_eq!(br.len(), 2, "a disc off the apex has two tangent rulings");
        let window = Interval {
            lo: br[0].hi.clone(),
            hi: br[1].lo.clone(),
        };
        let cut = match quadric_cut_loop(
            &chart,
            &surface,
            &window,
            &zero,
            24,
            &Q::from_i128(1),
            &DevConfig::tight(),
        ) {
            Verdict::Verified(l) => l,
            _ => panic!("the footprint loop must certify"),
        };

        // Junctions well inside the window, one on each branch.
        let mid = window.lo.add(&window.hi).mul(&Q::new(1, 2));
        let from = mid.add(&window.hi.sub(&mid).mul(&Q::new(1, 4)));
        let to = mid.add(&window.hi.sub(&mid).mul(&Q::new(1, 2)));
        let arc = tangent_turn_arc(&cut, &from, true, &to, false).expect("the loop turns");
        assert!(
            arc.len() >= 3,
            "a turn needs several pieces, got {}",
            arc.len()
        );

        let ends: Vec<(f64, f64)> = arc
            .iter()
            .map(|p| {
                let [a, _] = p.eval(&p.domain.lo).expect("evaluable");
                let [b, _] = p.eval(&p.domain.hi).expect("evaluable");
                (to_f64(&a), to_f64(&b))
            })
            .collect();

        // (2) exactly at the junctions.
        assert!(
            (ends[0].0 - to_f64(&from)).abs() < 1e-12,
            "the arc must start at the junction σ = {:.9}, got {:.9}",
            to_f64(&from),
            ends[0].0
        );
        assert!(
            (ends[ends.len() - 1].1 - to_f64(&to)).abs() < 1e-12,
            "and end at σ = {:.9}, got {:.9}",
            to_f64(&to),
            ends[ends.len() - 1].1
        );

        // (1) it reverses: σ rises, then falls.
        let rose = ends.iter().any(|(a, b)| b > a);
        let fell = ends.iter().any(|(a, b)| b < a);
        assert!(
            rose && fell,
            "the arc must turn around in σ — that is the whole reason it is not a rail: {ends:?}"
        );

        // (3) it contains the tangent, past both junctions.
        let peak = ends.iter().map(|(_, b)| *b).fold(f64::MIN, f64::max);
        assert!(
            peak > to_f64(&to) && peak > to_f64(&from),
            "the arc must reach the tangent (σ ≈ {:.6}), peaked at {peak:.6}",
            to_f64(&window.hi)
        );

        // **Both junctions on the same branch ⇒ the arc wraps BOTH tangents.** This is what a
        // contour bounding one whole side of a part needs: its two tangents are joined by one
        // continuous run of contour boundary, so there is no second junction to end at after the
        // first turn. Same call, one flag different — and it must reach *both* extremes, which the
        // one-turn arc above does not.
        //
        // The junctions run the *long* way round here — leaving the upper branch at the larger σ and
        // rejoining it at the smaller — which is the order a boundary traverses them in: the upper
        // chain hands over at its `σ_hi` end and gets the boundary back at its `σ_lo` one.
        let both = tangent_turn_arc(&cut, &to, true, &from, true).expect("the loop turns twice");
        let ends2: Vec<(f64, f64)> = both
            .iter()
            .map(|p| {
                let [a, _] = p.eval(&p.domain.lo).expect("evaluable");
                let [b, _] = p.eval(&p.domain.hi).expect("evaluable");
                (to_f64(&a), to_f64(&b))
            })
            .collect();
        let hi = ends2.iter().map(|(_, b)| *b).fold(f64::MIN, f64::max);
        let lo = ends2.iter().map(|(_, b)| *b).fold(f64::MAX, f64::min);
        assert!(
            hi > to_f64(&window.hi) - 1e-6 && lo < to_f64(&window.lo) + 1e-6,
            "a two-turn arc must reach both tangents (σ ≈ {:.6} and {:.6}), spanned [{lo:.6}, \
             {hi:.6}]",
            to_f64(&window.lo),
            to_f64(&window.hi)
        );
        assert!(
            both.len() > arc.len(),
            "and it must be the longer way round: {} pieces against the one-turn arc's {}",
            both.len(),
            arc.len()
        );
        // It still starts and ends exactly where it was told.
        assert!(
            (ends2[0].0 - to_f64(&to)).abs() < 1e-12
                && (ends2[ends2.len() - 1].1 - to_f64(&from)).abs() < 1e-12,
            "the two-turn arc must also land on its junctions exactly"
        );
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
        let profile = arrange2d::profile::Profile::new()
            .rect(cx.clone(), cy.clone(), h.clone(), h.clone())
            .into_edges();
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
        let loop_ = match shadow_cut_loops(
            &chart,
            &walls,
            inside,
            &window,
            &zero,
            16,
            &Q::from_i128(1),
            &cfg,
        ) {
            Verdict::Verified(mut l) => {
                assert_eq!(l.len(), 1, "a convex footprint traces one loop");
                l.remove(0)
            }
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

    /// **Operand size stays bounded along the certificate's chain (OPT.2.1).**
    ///
    /// `eval_ratfunc_on` rounds, so field components arrive ~18 digits — but rational addition
    /// MULTIPLIES denominators, so `chart_point_on`'s five further ops chained them to ~120 digits
    /// on every one of the thousands of sub-interval evaluations, and `metric_distance_on` then
    /// squared, summed and took a root of those, reaching 499 digits at `subdiv ≥ 64`. Measured:
    /// the cut-certificate path ran 8.5–9× slower for it, with `cut_evals` identical — pure
    /// cost-per-operation, invisible to a counter-based gate.
    ///
    /// This pins the fix. Without the `.rounded()` calls the numbers below go back to ~120 and
    /// ~500; the bound here is deliberately loose (40) so it fails on a regression, not on noise.
    #[test]
    fn the_certificate_chain_keeps_its_operands_bounded() {
        let chart = fixtures::devices::cone_wrap();
        let surface = CutSurface::Cylinder {
            axis_point: [Q::new(-1, 2), Q::new(27, 10), Q::from_i128(0)],
            axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
            r2: Q::new(1, 40),
        };
        let cfg = DevConfig::tight();
        let roots = crate::pcurve::scan_roots(
            &cut_mu_form(&chart, &surface, &Q::from_i128(0))
                .unwrap()
                .disc(),
            &Q::new(-5, 4),
            &Q::new(5, 4),
            512,
            60,
        )
        .expect("roots");
        let (lo, hi) = (roots[0].clone(), roots[1].clone());
        let mc = cut_mu_form(&chart, &surface, &Q::from_i128(0)).unwrap();
        let at = |t: &Q| {
            let s = crate::pcurve::snap(&lo.add(&hi.sub(&lo).mul(t)), 30);
            let (m, h) = mc.branch_at(&s, &cfg.sqrt_eps).expect("branch");
            (s, crate::pcurve::snap(&m.add(&h), 30))
        };
        let piece = segment(&at(&Q::new(4, 10)), &at(&Q::new(5, 10)));
        let dig = |r: &Q| {
            let (n, d) = r.numer_denom_decimal();
            n.len().max(d.len())
        };
        for n in [16usize, 32, 64, 128] {
            let width = piece
                .domain
                .hi
                .sub(&piece.domain.lo)
                .div(&Q::from_i128(n as i128));
            let (mut dt, mut ds, mut dm) = (0usize, 0usize, 0usize);
            let (mut dx, mut dd) = (0usize, 0usize);
            for k in 0..n {
                let t = subiv(&piece.domain.lo, &width, k);
                dt = dt.max(dig(t.lo()).max(dig(t.hi())));
                if let Some([sig, mu]) = piece.eval_on(&t) {
                    ds = ds.max(dig(sig.lo()).max(dig(sig.hi())));
                    dm = dm.max(dig(mu.lo()).max(dig(mu.hi())));
                    if let Some(x) = chart_point_on(&chart, &sig, &mu, &Q::from_i128(0)) {
                        for c in &x {
                            dx = dx.max(dig(c.lo()).max(dig(c.hi())));
                        }
                        let half = Q::new(1, 2);
                        if let DistOn::Bound(d) | DistOn::Loose(d) =
                            surface_distance_on(&surface, None, &x, &half, &cfg)
                        {
                            dd = dd.max(dig(&d));
                        }
                    }
                }
            }
            assert!(
                dx <= 40 && dd <= 40,
                "subdiv {n}: operands must stay bounded by the DEV.2a outward rounding — \
                 chart_point {dx} digits, distance {dd} digits (was 120 and 499)"
            );
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

    /// **The nappe condition is a statement about the ball, not about the DRC.** The same point,
    /// the same cone, the same authored nappe — only the *working radius* differs. At a radius
    /// wide enough to swallow the apex plane the selector cannot hold; at the radii the first-order
    /// bound actually closes on, it can, and the certificate is the same one it always was.
    ///
    /// This is what refuses a real cut when the two are conflated: the device's imported bore sits
    /// `3.61 mm` above its drafted cutter's apex plane and was checked against a fixed
    /// `clearance/2 = 3.5 mm`, so a wall with millimetres of headroom read as
    /// [`CutFitFault::NappeCrossed`] (#292). The loop tries `radius/16 … radius`, so a point this
    /// far out certifies on one of the smaller balls.
    #[test]
    fn a_wide_working_radius_does_not_by_itself_cross_the_nappe() {
        let (o, i) = (Q::from_i128(0), Q::from_i128(1));
        let m = [
            [i.clone(), o.clone(), o.clone()],
            [o.clone(), i.clone(), o.clone()],
            [o.clone(), o.clone(), Q::from_i128(-1)],
        ];
        let nappe = Nappe {
            n: [o.clone(), o.clone(), i.clone()],
            d: o.clone(),
        };
        let cfg = DevConfig::tight();
        // On the authored nappe, 4 above the apex plane — and 1/10 off the cone in x.
        let p = [Q::new(41, 10), Q::from_i128(0), Q::from_i128(4)];
        let x: [RatIv<Bignum>; 3] = core::array::from_fn(|k| RatIv::point(p[k].clone()));
        let probe = |radius: Q| {
            quadric_distance_on(
                &m,
                &[o.clone(), o.clone(), o.clone()],
                &o,
                &nappe,
                &x,
                &radius,
                &cfg,
            )
        };
        assert!(
            matches!(probe(Q::new(1, 10)), DistOn::Bound(_)),
            "a snug radius certifies"
        );
        assert!(
            matches!(probe(Q::from_i128(16)), DistOn::Bound(_)),
            "and so does a radius whose own ball reaches past the apex plane, because the ball the \
             bound closes on does not"
        );
        // The genuine case still bites: 16 subdivided four times is still 1, and the point sits 4
        // above the plane, so a point *at* the plane fails at every radius offered.
        let at_plane: [RatIv<Bignum>; 3] =
            core::array::from_fn(|k| RatIv::point([i.clone(), o.clone(), o.clone()][k].clone()));
        assert!(
            matches!(
                quadric_distance_on(
                    &m,
                    &[o.clone(), o.clone(), o.clone()],
                    &o,
                    &nappe,
                    &at_plane,
                    &Q::from_i128(16),
                    &cfg
                ),
                DistOn::Fault(CutFitFault::NappeCrossed)
            ),
            "a point on the selector plane crosses at every radius"
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

    /// **The event polynomials must stay low-degree, and that is a correctness-shaped budget rather
    /// than a speed preference.** These families are products of the chart's own rational fields, so
    /// they arrive carrying its denominator several times over: on this square prism a raw pairwise
    /// resultant is degree **78**, and its reduced form is degree **4**. Sturm's chain is a naive
    /// ℚ-PRS, so that difference is the whole cost of the event set (measured 273 ms → 16 ms).
    ///
    /// Asserted as a degree rather than a duration, for the reason VV.1 counts work instead of
    /// timing it: a wall-clock threshold flakes, and a degree does not.
    #[test]
    fn the_event_polynomials_stay_low_degree() {
        let chart = fixtures::devices::cone_wrap();
        let (cx, cy, h) = (Q::new(-1, 2), Q::new(27, 10), Q::new(1, 8));
        let profile = arrange2d::profile::Profile::new()
            .rect(cx.clone(), cy.clone(), h.clone(), h.clone())
            .into_edges();
        let cast = crate::extrude::Cast::new(
            xy_frame(),
            crate::extrude::Apex::direction([Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)])
                .expect("a real direction"),
        )
        .expect("the apex is off the frame plane");
        let walls = cast.carrier_walls(&profile).expect("four distinct lines");
        let zero = Q::from_i128(0);
        let forms: Vec<MuCut<Bignum>> = walls
            .iter()
            .map(|w| cut_mu_form(&chart, w, &zero).expect("each wall pulls back"))
            .collect();
        for (i, f) in forms.iter().enumerate() {
            for (j, g) in forms.iter().enumerate().skip(i + 1) {
                let raw = f.resultant(g);
                let deg = raw.reduce().num().degree().unwrap_or(0);
                assert!(
                    deg <= 8,
                    "the reduced resultant of walls {i} and {j} must stay low-degree, got {deg} \
                     (raw {:?})",
                    raw.num().degree()
                );
            }
        }
    }

    // ── AUTH.2b: every inside stretch ───────────────────────────────────────────────────────

    /// An L-shaped profile at `(cx, cy)`, CCW, arm `1/4` and thickness `1/8` — one reflex corner,
    /// so a ruling can meet it in two stretches. The shape AUTH.2 exists for.
    fn l_profile(cx: &Q, cy: &Q) -> Vec<geom::content::Edge<Bignum>> {
        let (arm, th, z) = (Q::new(1, 4), Q::new(1, 8), Q::from_i128(0));
        let pt = |dx: &Q, dy: &Q| [cx.add(dx), cy.add(dy)];
        arrange2d::profile::Profile::new()
            .polygon(&[
                pt(&z, &z),
                pt(&arm, &z),
                pt(&arm, &th),
                pt(&th, &th),
                pt(&th, &arm),
                pt(&z, &arm),
            ])
            .into_edges()
    }

    /// A **keyhole** at `(cx, cy)`: a round head of radius `1/10` with a straight stem hanging off
    /// the chord at `−12/125`, whose sides `±7/250` meet the circle exactly (`7² + 24² = 25²`), the
    /// whole thing laid out on the rotated axes `u = (15/17, 8/17)`, `v = (−8/17, 15/17)`.
    ///
    /// The L is all straight lines; this one mixes a **quadric** wall with affine ones, which is
    /// what makes its stretch structure different in kind. Where the L's two stretches are born and
    /// die separately, a ruling crossing the keyhole obliquely enters the head, leaves it through
    /// the notch beside the stem, and re-enters the stem — two stretches that **merge** as the
    /// ruling turns, over a saddle where one bounding wall is the circle and the other a stem side.
    ///
    /// Both rational choices are load-bearing rather than decorative. The **stem is narrow** (`7/25`
    /// of the head radius, not `3/5`) because the notch beside it is what a ruling has to pass
    /// through to see two stretches, and a wide stem nearly closes it: measured over the same
    /// 800-ruling sweep, the wide stem gave 9 two-stretch rulings and this one 14. The **rotation**
    /// is chosen for the same reason — the ruling has to cross the notch *obliquely*, and the
    /// unrotated keyhole gave 8. Neither is a free choice, which is §11.6 again: a fixture that
    /// merely looks like the phenomenon does not produce it.
    fn keyhole_profile(cx: &Q, cy: &Q) -> Vec<geom::content::Edge<Bignum>> {
        let (ux, uy) = (Q::new(15, 17), Q::new(8, 17));
        let (vx, vy) = (Q::new(-8, 17), Q::new(15, 17));
        let pt = |su: &Q, sv: &Q| {
            [
                cx.add(&ux.mul(su)).add(&vx.mul(sv)),
                cy.add(&uy.mul(su)).add(&vy.mul(sv)),
            ]
        };
        let (hw, chord, foot) = (Q::new(7, 250), Q::new(12, 125), Q::new(1, 5));
        let a = pt(&hw, &chord.clone().neg());
        let b = pt(&hw.clone().neg(), &chord.neg());
        let c = pt(&hw.clone().neg(), &foot.clone().neg());
        let d = pt(&hw, &foot.neg());
        arrange2d::profile::Profile::new()
            .arc(cx.clone(), cy.clone(), Q::new(1, 100), a.clone(), b.clone())
            .polyline(&[b, c, d, a])
            .into_edges()
    }

    /// The pieces a multi-walled cutter needs: its walls' µ̂-forms, the fill rule, and a window —
    /// for a profile drawn at `(cx, cy)` and swept along `z` over the wrapping cone.
    #[allow(clippy::type_complexity)]
    fn profile_cutter(
        cx: Q,
        cy: Q,
        profile: Vec<geom::content::Edge<Bignum>>,
    ) -> (
        geom::chart::Chart<Bignum>,
        Vec<MuCut<Bignum>>,
        crate::extrude::Cast<Bignum>,
        Vec<geom::content::Edge<Bignum>>,
        Interval<Bignum>,
    ) {
        let chart = fixtures::devices::cone_wrap();
        let cast = crate::extrude::Cast::new(
            xy_frame(),
            crate::extrude::Apex::direction([Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)])
                .expect("a real direction"),
        )
        .expect("the apex is off the frame plane");
        let walls = cast.carrier_walls(&profile).expect("distinct carriers");
        let zero = Q::from_i128(0);
        let forms: Vec<MuCut<Bignum>> = walls
            .iter()
            .map(|w| cut_mu_form(&chart, w, &zero).expect("each wall pulls back"))
            .collect();
        let window = bounding_window(
            &chart,
            &CutSurface::Cylinder {
                axis_point: [cx, cy, Q::from_i128(0)],
                axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
                r2: Q::from_i128(1),
            },
        );
        (chart, forms, cast, profile, window)
    }

    /// The pieces an L-profile cutter needs: its walls, their µ̂-forms, the fill rule, and a window.
    #[allow(clippy::type_complexity)]
    fn l_cutter() -> (
        geom::chart::Chart<Bignum>,
        Vec<MuCut<Bignum>>,
        crate::extrude::Cast<Bignum>,
        Vec<geom::content::Edge<Bignum>>,
        Interval<Bignum>,
    ) {
        let (cx, cy) = (Q::new(-1, 2), Q::new(27, 10));
        let profile = l_profile(&cx, &cy);
        profile_cutter(cx, cy, profile)
    }

    /// **A ruling that meets a non-convex cutter twice reports both stretches — and the band still
    /// refuses it.** The engine reads the footprint; the band's scope restriction is one line in the
    /// sugar over it, which is what lets AUTH.2c replace the caller without touching the reader.
    ///
    /// The two stretches are shown to be *genuinely* separate rather than an artifact: the fill rule
    /// says **outside** at the midpoint of the gap between them.
    #[test]
    fn a_non_convex_ruling_reports_both_stretches_while_the_band_refuses() {
        let (chart, forms, cast, profile, window) = l_cutter();
        let cfg = DevConfig::tight();
        let zero = Q::from_i128(0);
        let inside = |s: &Q, mu: &Q| -> Option<bool> {
            let p = chart.surface(mu, &zero).eval(s)?;
            cast.contains(&p, &profile)
        };
        const SCAN: i128 = 400;
        let mut double: Option<(Q, Vec<RulingPatch<Bignum>>)> = None;
        let mut single = 0;
        for k in 0..=SCAN {
            let s = window
                .lo
                .add(&window.hi.sub(&window.lo).mul(&Q::new(k, SCAN)));
            let Ok(ps) = ruling_patches(&forms, &s, &inside, &cfg.sqrt_eps) else {
                continue;
            };
            match ps.len() {
                0 => {}
                1 => single += 1,
                _ => {
                    if double.is_none() {
                        double = Some((s, ps));
                    }
                }
            }
        }
        let (s, ps) = double.expect("some ruling must cross both arms of the L");
        assert_eq!(
            ps.len(),
            2,
            "an L is two convex arms: at most two stretches"
        );
        assert!(single > 0, "and most rulings still meet it once");

        // The gap between them is really outside — otherwise this is one stretch misreported.
        let gap_mid = ps[0].hi.add(&ps[1].lo).mul(&Q::new(1, 2));
        assert_eq!(
            inside(&s, &gap_mid),
            Some(false),
            "the two stretches must be separated by material the cutter does not cover"
        );
        assert!(
            ps[0].hi.cmp(&ps[1].lo) == core::cmp::Ordering::Less,
            "and ordered in µ̂"
        );

        // Both ends of each stretch name the wall that produced them, so a traced piece knows what
        // to certify against — the property the band reader used to guarantee by construction.
        for p in &ps {
            for (wi, _) in [p.lo_at, p.hi_at] {
                assert!(wi < forms.len(), "each end names a real wall");
            }
        }
    }

    /// **The exact event set buys tightness, not soundness — starve the sampling and ε degrades
    /// while the geometry stays honest.**
    ///
    /// §11.5's claim is that nothing rests on the event set being complete: the sweep applies the
    /// same matching between *every* consecutive pair of columns, so an event it stepped over is a
    /// sampling loss and shows up as a loose bound, never as a quietly wrong hole. That is a claim
    /// about code, and until something checks it, it is only a claim.
    ///
    /// The perturbation is applied to the **columns** rather than to the event list, and that is
    /// the faithful form of it: the events do not enter the matching at all — they only decide
    /// where columns are placed — so a starved column set *is* a stepped-over event, and it is the
    /// perturbation the engine can actually be subjected to from outside.
    ///
    /// Two things are then measured. **ε degrades**: the starved loop's certified bound is strictly
    /// worse, which is what "the search buys tightness" means quantitatively. **The geometry stays
    /// honest**: every emitted piece is re-checked, here and independently of the builder, at its
    /// own σ-midpoint against the boundary the exact fill rule reports there — and the deviation is
    /// inside the starved loop's *own* ε. A loop that had drifted off the true cut would fail that
    /// while certifying perfectly, which is the failure this exists to exclude.
    #[test]
    fn starving_the_sweep_loosens_eps_and_leaves_the_geometry_honest() {
        let (cx, cy) = (Q::new(-1, 2), Q::new(27, 10));
        let profile = l_profile(&cx, &cy);
        let (chart, forms, cast, profile, window) = profile_cutter(cx, cy, profile);
        let walls = cast.carrier_walls(&profile).expect("six distinct lines");
        let cfg = DevConfig::tight();
        let zero = Q::from_i128(0);
        let clearance = Q::from_i128(4);
        let inside = |s: &Q, mu: &Q| -> Option<bool> {
            let p = chart.surface(mu, &zero).eval(s)?;
            cast.contains(&p, &profile)
        };
        let trace = |segments: usize| match shadow_cut_loops(
            &chart, &walls, inside, &window, &zero, segments, &clearance, &cfg,
        ) {
            Verdict::Verified(ls) => ls,
            other => panic!(
                "the L must trace at {segments} segments: {}",
                match other {
                    Verdict::Refuted(f) => format!("Refuted({f:?})"),
                    _ => "Unresolved".into(),
                }
            ),
        };
        let fine = trace(24);
        let coarse = trace(2);
        let worst = |ls: &[CutLoop<Bignum>]| {
            ls.iter()
                .map(|l| l.eps.clone())
                .max_by(|a, b| a.cmp(b))
                .expect("a loop")
        };
        let (e_fine, e_coarse) = (worst(&fine), worst(&coarse));
        assert!(
            e_fine.cmp(&e_coarse) == core::cmp::Ordering::Less,
            "starving the sweep must LOOSEN the bound: fine {:?} vs coarse {:?}",
            crate::pcurve::snap(&e_fine, 20),
            crate::pcurve::snap(&e_coarse, 20),
        );

        // …and the starved geometry is still where the fill rule says the boundary is. Checked at
        // each piece's own σ-midpoint against `ruling_patches`, which is the exact reader — not
        // against the fine loop, which would only say the two searches agree.
        let mut checked = 0;
        for l in &coarse {
            for pc in &l.pieces {
                let (a, b) = (
                    pc.sigma
                        .eval(&pc.domain.lo)
                        .expect("a straight piece evaluates"),
                    pc.sigma
                        .eval(&pc.domain.hi)
                        .expect("a straight piece evaluates"),
                );
                let sm = a.add(&b).mul(&Q::new(1, 2));
                let Ok(truth) = ruling_patches(&forms, &sm, &inside, &cfg.sqrt_eps) else {
                    continue;
                };
                let mut bounds: Vec<Q> = Vec::new();
                for p in &truth {
                    bounds.push(p.lo.clone());
                    bounds.push(p.hi.clone());
                }
                if bounds.is_empty() {
                    continue;
                }
                for mu in loop_mu_at(core::slice::from_ref(pc), &sm) {
                    let d = bounds
                        .iter()
                        .map(|t| {
                            let d = mu.sub(t);
                            if d.sign() < 0 { d.neg() } else { d }
                        })
                        .min_by(|x, y| x.cmp(y))
                        .expect("a boundary");
                    assert!(
                        d.cmp(&e_coarse) != core::cmp::Ordering::Greater,
                        "a starved piece sits {:?} from the fill rule's own boundary at σ = {:?}, \
                         outside its own certified {:?} — the bound got loose AND the geometry \
                         moved, which is the failure §11.5 claims cannot happen",
                        crate::pcurve::snap(&d, 20),
                        crate::pcurve::snap(&sm, 20),
                        crate::pcurve::snap(&e_coarse, 20),
                    );
                    checked += 1;
                }
            }
        }
        assert!(
            checked >= 8,
            "only {checked} starved pieces were re-checked"
        );
    }

    /// **A keyhole's two stretches rejoin over a saddle where a circle meets a straight edge.**
    ///
    /// A merge is the event whose two ends belong to *different* stretches: what closes is the
    /// **gap** between them, not either one's width. That much the L already exercises — its two
    /// arms rejoin at the reflex corner, and a first version of this test asserted only the closing
    /// gap and passed unchanged when pointed at the L, which is why the claim is narrower now.
    ///
    /// What is the keyhole's own is **which walls bound the closing gap**: one is the head's circle
    /// and the other a stem side, so the saddle is the mixed quadric-against-affine case of
    /// [`MuCut::resultant`] — the one an all-straight profile can never reach, and the one §11.2's
    /// criterion asks for by name ("the test set must contain the case the feature is for", and the
    /// published quadratic-by-quadratic closed form is *identically zero* on two affine walls).
    /// Asserted by reading the wall each end names and checking their pullbacks' degrees differ.
    #[test]
    fn a_keyhole_sweep_closes_a_gap_between_a_circle_and_a_straight_edge() {
        let (cx, cy) = (Q::new(-1, 2), Q::new(27, 10));
        let profile = keyhole_profile(&cx, &cy);
        let (chart, forms, cast, profile, window) = profile_cutter(cx, cy, profile);
        let cfg = DevConfig::tight();
        let zero = Q::from_i128(0);
        let inside = |s: &Q, mu: &Q| -> Option<bool> {
            let p = chart.surface(mu, &zero).eval(s)?;
            cast.contains(&p, &profile)
        };
        const SCAN: i128 = 800;
        let at = |k: i128| {
            window
                .lo
                .add(&window.hi.sub(&window.lo).mul(&Q::new(k, SCAN)))
        };
        // Sweep, and keep the two-stretch column immediately before each drop to one: there the gap
        // between the stretches is about to close, and the two walls facing across it are the ones
        // the saddle joins.
        let mut merges: Vec<(Q, Q, usize, usize)> = Vec::new(); // (gap, narrowest stretch, walls)
        let mut prev: Option<Vec<RulingPatch<Bignum>>> = None;
        let mut two = 0;
        for k in 0..=SCAN {
            let s = at(k);
            let Ok(ps) = ruling_patches(&forms, &s, &inside, &cfg.sqrt_eps) else {
                prev = None;
                continue;
            };
            if ps.len() == 2 {
                two += 1;
            }
            if ps.len() == 1
                && let Some(p2) = prev.as_ref()
                && p2.len() == 2
            {
                let gap = p2[1].lo.sub(&p2[0].hi);
                let w0 = p2[0].hi.sub(&p2[0].lo);
                let w1 = p2[1].hi.sub(&p2[1].lo);
                let narrow = if w0.cmp(&w1) == core::cmp::Ordering::Less {
                    w0
                } else {
                    w1
                };
                merges.push((gap, narrow, p2[0].hi_at.0, p2[1].lo_at.0));
            }
            prev = Some(ps);
        }
        assert!(
            two > 0,
            "some ruling must cross the head and the stem separately — the fixture does not \
             produce the phenomenon"
        );
        let (gap, narrow, wa, wb) = merges
            .iter()
            .min_by(|a, b| a.0.cmp(&b.0))
            .expect("the two stretches must rejoin somewhere in the window");
        // What closed is the gap, not a stretch — the signature of a merge rather than a death.
        assert!(
            gap.sign() > 0 && gap.cmp(&narrow.mul(&Q::new(1, 5))) == core::cmp::Ordering::Less,
            "the stretches must rejoin by their gap closing (gap {:?}, narrowest stretch {:?})",
            crate::pcurve::snap(gap, 20),
            crate::pcurve::snap(narrow, 20),
        );
        // …and the two walls facing across it are of **different degree**: the head's circle pulls
        // back to a genuine µ̂-quadratic, a stem side to an affine form. This is the assertion the
        // L cannot satisfy — every wall of a polygon is affine — and it is the mixed case of the
        // pairwise resultant, where the published quadratic-by-quadratic form is not identically
        // zero.
        let deg = |wi: usize| if forms[wi].a.is_zero() { 1 } else { 2 };
        assert_ne!(
            deg(*wa),
            deg(*wb),
            "the saddle must join the circle to a stem side (walls {wa} and {wb}, degrees {} and \
             {}) — two walls of equal degree means the fixture is exercising the L's case again",
            deg(*wa),
            deg(*wb),
        );
    }

    /// **A carrier crossing the cutter's own interior is not a boundary, and must not split a
    /// stretch.** A carrier is the whole infinite line, not the profile edge on it, so a non-convex
    /// profile has carriers running through its own interior — the L's `y = 1` bounds one arm and is
    /// interior to the other. Those crossings arrive in the sorted list like any other, and reported
    /// as-is they break one stretch into two abutting ones: measured before the fix, this L reported
    /// **three** stretches on some rulings, which a straight line meeting two convex arms cannot do.
    ///
    /// Convex profiles cannot exhibit it — their carriers are supporting lines, so every extra
    /// crossing falls outside the inside stretch — which is exactly why AUTH.1e.4 never saw it.
    #[test]
    fn a_carrier_crossing_the_interior_does_not_split_a_stretch() {
        let (chart, forms, cast, profile, window) = l_cutter();
        let cfg = DevConfig::tight();
        let zero = Q::from_i128(0);
        let inside = |s: &Q, mu: &Q| -> Option<bool> {
            let p = chart.surface(mu, &zero).eval(s)?;
            cast.contains(&p, &profile)
        };
        const SCAN: i128 = 400;
        let mut interior_crossings = 0;
        for k in 0..=SCAN {
            let s = window
                .lo
                .add(&window.hi.sub(&window.lo).mul(&Q::new(k, SCAN)));
            let Ok(ps) = ruling_patches(&forms, &s, &inside, &cfg.sqrt_eps) else {
                continue;
            };
            // No stretch may report more than the two an L can offer, at any ruling.
            assert!(
                ps.len() <= 2,
                "an L cannot meet a straight ruling in {} stretches — a carrier crossing its own \
                 interior was reported as a boundary",
                ps.len()
            );
            // Count the crossings that fall strictly *inside* a reported stretch: each is a
            // phantom boundary the merge had to absorb.
            for form in &forms {
                let Some(roots) = form.roots_at(&s, &cfg.sqrt_eps) else {
                    continue;
                };
                for (mu, _) in roots {
                    if ps.iter().any(|p| {
                        p.lo.cmp(&mu) == core::cmp::Ordering::Less
                            && mu.cmp(&p.hi) == core::cmp::Ordering::Less
                    }) {
                        interior_crossings += 1;
                    }
                }
            }
        }
        assert!(
            interior_crossings > 0,
            "this fixture must actually exercise the case — no carrier crossed the L's interior"
        );
    }

    // ── AUTH.2c: the tracer ─────────────────────────────────────────────────────────────────

    /// The square-prism cutter of AUTH.1e.4 — walls, fill rule and the bounding circle's window.
    #[allow(clippy::type_complexity)]
    fn square_cutter() -> (
        geom::chart::Chart<Bignum>,
        Vec<CutSurface<Bignum>>,
        crate::extrude::Cast<Bignum>,
        Vec<geom::content::Edge<Bignum>>,
        Interval<Bignum>,
    ) {
        let chart = fixtures::devices::cone_wrap();
        let (cx, cy, h) = (Q::new(-1, 2), Q::new(27, 10), Q::new(1, 8));
        let profile = arrange2d::profile::Profile::new()
            .rect(cx.clone(), cy.clone(), h.clone(), h.clone())
            .into_edges();
        let cast = crate::extrude::Cast::new(
            xy_frame(),
            crate::extrude::Apex::direction([Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)])
                .expect("a real direction"),
        )
        .expect("the apex is off the frame plane");
        let walls = cast.carrier_walls(&profile).expect("four distinct lines");
        let window = bounding_window(
            &chart,
            &CutSurface::Cylinder {
                axis_point: [cx, cy, Q::from_i128(0)],
                axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
                r2: h.mul(&h).mul(&Q::from_i128(2)),
            },
        );
        (chart, walls, cast, profile, window)
    }

    /// **The traced bound on the square prism stays pinned.** AUTH.2c's differential ran the
    /// retired band builder and the tracer over this same fixture and compared the emitted
    /// boundaries; it certified the generalization and licensed replacing the caller, and then the
    /// band builder was deleted rather than kept alive to protect its own test.
    ///
    /// What survives it is this: at `segments = 16` the band reached `ε = 2.241e-3` and the tracer
    /// `3.104e-3`, so a ceiling of `6e-3` is comfortable headroom over the measurement and still
    /// well under the 8× regression that dropping the grid-adjacent cell-end nodes produces
    /// (`1.79e-2`, mutation-verified). The number's provenance is an implementation that no longer
    /// exists, which is worth saying out loud — but the *geometry* is still checked against
    /// something independent, by the inscribed/circumscribed sandwich test above.
    #[test]
    fn the_traced_bound_on_the_square_prism_stays_pinned() {
        let (chart, walls, cast, profile, window) = square_cutter();
        let zero = Q::from_i128(0);
        let inside = |s: &Q, mu: &Q| -> Option<bool> {
            let p = chart.surface(mu, &zero).eval(s)?;
            cast.contains(&p, &profile)
        };
        let traced = match shadow_cut_loops(
            &chart,
            &walls,
            inside,
            &window,
            &zero,
            16,
            &Q::from_i128(1),
            &DevConfig::tight(),
        ) {
            Verdict::Verified(l) => l,
            other => panic!("the tracer must certify: {:?}", verdict_tag(&other)),
        };
        assert_eq!(traced.len(), 1, "a convex footprint is one loop");
        assert!(
            traced[0].eps.cmp(&Q::new(6, 1000)) == core::cmp::Ordering::Less,
            "the traced bound must stay at the measured scale, got {:.3e}",
            to_f64(&traced[0].eps)
        );
    }

    /// **A non-convex footprint traces one closed loop that a band cannot express.** The L's loop
    /// must *turn around in σ*: at a ruling through the notch it has **four** boundary points, not
    /// two, which is exactly the shape a near/far rail pair has no way to carry.
    #[test]
    fn the_l_slot_traces_one_loop_that_turns_around_in_sigma() {
        let (chart, _forms, cast, profile, window) = l_cutter();
        let walls = cast.carrier_walls(&profile).expect("six distinct lines");
        let cfg = DevConfig::tight();
        let zero = Q::from_i128(0);
        let inside = |s: &Q, mu: &Q| -> Option<bool> {
            let p = chart.surface(mu, &zero).eval(s)?;
            cast.contains(&p, &profile)
        };
        let traced = match shadow_cut_loops(
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
            other => panic!("the L must trace: {:?}", verdict_tag(&other)),
        };
        assert_eq!(traced.len(), 1, "a connected L is one loop");
        // Somewhere across the window the loop is met four times by a ruling.
        const SCAN: i128 = 400;
        let mut four = 0;
        for k in 0..=SCAN {
            let s = window
                .lo
                .add(&window.hi.sub(&window.lo).mul(&Q::new(k, SCAN)));
            if loop_mu_at(&traced[0].pieces, &s).len() >= 4 {
                four += 1;
            }
        }
        assert!(
            four > 0,
            "the traced loop must double back through the notch — a band never does"
        );
    }

    /// **Refining `segments` tightens the traced bound.** The contract the whole milestone rests on
    /// is that a loose loop is `Unresolved` and refinable, never a quietly wrong one, so ε must
    /// actually respond to the knob on the shape that exercises the tracer's own machinery.
    #[test]
    fn the_traced_bound_refines_with_segments() {
        let (chart, _forms, cast, profile, window) = l_cutter();
        let walls = cast.carrier_walls(&profile).expect("six distinct lines");
        let cfg = DevConfig::tight();
        let zero = Q::from_i128(0);
        let inside = |s: &Q, mu: &Q| -> Option<bool> {
            let p = chart.surface(mu, &zero).eval(s)?;
            cast.contains(&p, &profile)
        };
        let eps_at = |segs: usize| -> Q {
            match shadow_cut_loops(
                &chart,
                &walls,
                inside,
                &window,
                &zero,
                segs,
                &Q::from_i128(1),
                &cfg,
            ) {
                Verdict::Verified(l) => l[0].eps.clone(),
                other => panic!("the L must trace at {segs}: {:?}", verdict_tag(&other)),
            }
        };
        let (coarse, fine) = (eps_at(8), eps_at(32));
        assert!(
            fine.cmp(&coarse) == core::cmp::Ordering::Less,
            "ε must tighten with segments: {:.3e} at 8 vs {:.3e} at 32",
            to_f64(&coarse),
            to_f64(&fine)
        );
    }

    /// **A ring is refused as a nested loop — by name, and for its own reason.** The tracer has no
    /// trouble *tracing* an annulus: it is two loops, one inside the other. What makes it a refusal
    /// is the geometry it would describe — a through-cut leaving a disc of material floating free,
    /// which is two parts rather than one hole (§11.6).
    #[test]
    fn a_ring_is_refused_as_nested_rather_than_traced() {
        let chart = fixtures::devices::cone_wrap();
        let (cx, cy) = (Q::new(-1, 2), Q::new(27, 10));
        let profile = arrange2d::profile::Profile::new()
            .circle(cx.clone(), cy.clone(), Q::new(1, 4))
            .circle(cx.clone(), cy.clone(), Q::new(1, 8))
            .into_edges();
        let cast = crate::extrude::Cast::new(
            xy_frame(),
            crate::extrude::Apex::direction([Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)])
                .expect("a real direction"),
        )
        .expect("the apex is off the frame plane");
        let walls = cast.carrier_walls(&profile).expect("two distinct circles");
        // A strict superset of the ring's own window, so the footprint closes *inside* it: the
        // outer circle's tangent-to-tangent window is exactly filled by its own footprint, which
        // the tracer rightly reads as reaching the edge (`ShadowUnbounded`) before it ever gets to
        // ask about nesting.
        let window = bounding_window(
            &chart,
            &CutSurface::Cylinder {
                axis_point: [cx.clone(), cy.clone(), Q::from_i128(0)],
                axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
                r2: Q::new(1, 4),
            },
        );
        let zero = Q::from_i128(0);
        let inside = |s: &Q, mu: &Q| -> Option<bool> {
            let p = chart.surface(mu, &zero).eval(s)?;
            cast.contains(&p, &profile)
        };
        match shadow_cut_loops(
            &chart,
            &walls,
            inside,
            &window,
            &zero,
            16,
            &Q::from_i128(1),
            &DevConfig::tight(),
        ) {
            Verdict::Refuted(CutFitFault::ShadowNested) => {}
            other => panic!(
                "a ring must be refused as nested, got {:?}",
                verdict_tag(&other)
            ),
        }
    }

    // ── AUTH.2a: the exact event set ────────────────────────────────────────────────────────

    /// A µ̂-form from constant coefficients.
    fn form(a: i128, b: i128, c: i128) -> MuCut<Bignum> {
        let k = |v: i128| RatFunc::from_poly(Poly::constant(Q::from_i128(v)));
        MuCut {
            a: k(a),
            b: k(b),
            c: k(c),
        }
    }
    /// A µ̂-form whose coefficients are polynomials in σ (low-order coefficient first).
    fn form_poly(a: &[i128], b: &[i128], c: &[i128]) -> MuCut<Bignum> {
        let p = |v: &[i128]| {
            RatFunc::from_poly(Poly::from_coeffs(
                v.iter().map(|k| Q::from_i128(*k)).collect(),
            ))
        };
        MuCut {
            a: p(a),
            b: p(b),
            c: p(c),
        }
    }

    /// The 4×4 Sylvester determinant of two µ̂-quadratics at a σ — the textbook resultant, expanded
    /// by permutations, sharing no line with [`MuCut::resultant`]'s closed form.
    fn sylvester4(f: (Q, Q, Q), g: (Q, Q, Q)) -> Q {
        let m = [
            [f.0.clone(), f.1.clone(), f.2.clone(), Q::from_i128(0)],
            [Q::from_i128(0), f.0, f.1, f.2],
            [g.0.clone(), g.1.clone(), g.2.clone(), Q::from_i128(0)],
            [Q::from_i128(0), g.0, g.1, g.2],
        ];
        let perms: [([usize; 4], i128); 24] = [
            ([0, 1, 2, 3], 1),
            ([0, 1, 3, 2], -1),
            ([0, 2, 1, 3], -1),
            ([0, 2, 3, 1], 1),
            ([0, 3, 1, 2], 1),
            ([0, 3, 2, 1], -1),
            ([1, 0, 2, 3], -1),
            ([1, 0, 3, 2], 1),
            ([1, 2, 0, 3], 1),
            ([1, 2, 3, 0], -1),
            ([1, 3, 0, 2], -1),
            ([1, 3, 2, 0], 1),
            ([2, 0, 1, 3], 1),
            ([2, 0, 3, 1], -1),
            ([2, 1, 0, 3], -1),
            ([2, 1, 3, 0], 1),
            ([2, 3, 0, 1], 1),
            ([2, 3, 1, 0], -1),
            ([3, 0, 1, 2], -1),
            ([3, 0, 2, 1], 1),
            ([3, 1, 0, 2], 1),
            ([3, 1, 2, 0], -1),
            ([3, 2, 0, 1], -1),
            ([3, 2, 1, 0], 1),
        ];
        let mut total = Q::from_i128(0);
        for (perm, sign) in perms {
            let mut prod = Q::from_i128(sign);
            for (row, col) in perm.iter().enumerate() {
                prod = prod.mul(&m[row][*col]);
            }
            total = total.add(&prod);
        }
        total
    }

    /// **The resultant means what it claims, checked two independent ways.** For genuine quadratics
    /// it must equal the 4×4 Sylvester determinant; and at every µ̂-degree it must vanish *exactly*
    /// when the two walls cross the ruling at a common µ̂ — which is the property the tracer relies
    /// on, and the only one that survives the degenerate cases.
    #[test]
    fn the_resultant_vanishes_exactly_when_two_walls_share_a_crossing() {
        let zero = Q::from_i128(0);
        let val = |f: &MuCut<Bignum>| {
            (
                f.a.eval(&zero).unwrap(),
                f.b.eval(&zero).unwrap(),
                f.c.eval(&zero).unwrap(),
            )
        };
        // Quadratic × quadratic: agree with Sylvester, sharing a root or not.
        for (f, g) in [
            (form(1, -4, 3), form(1, -1, -6)), // (µ−3)(µ−1), (µ−3)(µ+2): share µ = 3
            (form(1, -4, 3), form(1, -5, 6)),  // (µ−3)(µ−1), (µ−3)(µ−2): share µ = 3
            (form(1, -3, 2), form(1, -9, 20)), // {1,2} and {4,5}: share nothing
            (form(2, 1, -6), form(3, -2, -1)), // arbitrary
        ] {
            let closed = f.resultant(&g).eval(&zero).unwrap();
            assert_eq!(
                closed,
                sylvester4(val(&f), val(&g)),
                "the closed form must be the Sylvester determinant"
            );
        }
        // Quadratic × affine, and — the case the four-term form gets wrong — affine × affine.
        // `2µ − 6` shares µ = 3 with `(µ−3)(µ+2)`; `2µ − 4` does not.
        assert_eq!(
            form(0, 2, -6)
                .resultant(&form(1, -1, -6))
                .eval(&zero)
                .unwrap()
                .sign(),
            0
        );
        assert_ne!(
            form(0, 2, -4)
                .resultant(&form(1, -1, -6))
                .eval(&zero)
                .unwrap()
                .sign(),
            0
        );
        // µ = 1 against µ = 1 (meeting) and against µ = 2 (never meeting).
        assert_eq!(
            form(0, 1, -1)
                .resultant(&form(0, 3, -3))
                .eval(&zero)
                .unwrap()
                .sign(),
            0,
            "two affine walls crossing at the same µ̂ must resolve as meeting"
        );
        assert_ne!(
            form(0, 1, -1)
                .resultant(&form(0, 1, -2))
                .eval(&zero)
                .unwrap()
                .sign(),
            0,
            "two affine walls that never meet must not read as meeting everywhere — the 4×4 \
             Sylvester form of degree-2-padded affine walls is identically zero, which would \
             erase every corner of a polygonal profile"
        );
    }

    /// **Two affine walls' meeting σ is located exactly.** `µ = σ` and `µ = 1 − σ` cross at
    /// `σ = 1/2` and nowhere else — the elementary shape of a polygon corner, where the wall
    /// governing the footprint's boundary changes.
    #[test]
    fn an_affine_pair_brackets_the_sigma_where_their_crossings_coincide() {
        let walls = [
            form_poly(&[], &[1], &[0, -1]), // µ − σ
            form_poly(&[], &[1], &[-1, 1]), // µ + σ − 1
        ];
        let tol = Q::new(1, 1 << 20);
        let events = structure_events(&walls, &ivl(0, 1), &tol).expect("the chains must verify");
        let meets: Vec<&StructureEvent<Bignum>> = events
            .iter()
            .filter(|e| e.kinds.contains(&EventKind::Meet(0, 1)))
            .collect();
        assert_eq!(meets.len(), 1, "exactly one meeting σ in [0, 1]");
        let at = &meets[0].at;
        assert!(
            at.lo.cmp(&Q::new(1, 2)) != core::cmp::Ordering::Greater
                && Q::new(1, 2).cmp(&at.hi) != core::cmp::Ordering::Greater,
            "the bracket must contain σ = 1/2, got [{}, {}]",
            to_f64(&at.lo),
            to_f64(&at.hi)
        );
        assert!(
            at.hi.sub(&at.lo).cmp(&tol) != core::cmp::Ordering::Greater,
            "and must be refined to the requested tolerance"
        );
    }

    /// **A double event is located, though nothing changes sign there.** A form whose discriminant
    /// is `(σ−1)²` touches zero without crossing it, so a sign-change scan — the tool AUTH.1e.4
    /// uses — steps straight over it. Sturm counts *distinct* roots, so the bisection tracks the
    /// count rather than the sign and finds it.
    #[test]
    fn a_tangential_event_is_found_where_a_sign_scan_would_miss_it() {
        // a = 1, b = 0, c = −(σ−1)²/4  ⇒  disc = b² − 4ac = (σ−1)².
        let c = Poly::from_coeffs(vec![Q::new(-1, 4), Q::new(1, 2), Q::new(-1, 4)]);
        let wall = MuCut {
            a: RatFunc::from_poly(Poly::constant(Q::from_i128(1))),
            b: RatFunc::from_poly(Poly::constant(Q::from_i128(0))),
            c: RatFunc::from_poly(c.clone()),
        };
        // The premise: the discriminant really does touch without crossing.
        let disc = wall.disc();
        for s in [Q::new(1, 2), Q::new(3, 2)] {
            assert!(
                disc.eval(&s).unwrap().sign() > 0,
                "the discriminant is positive on both sides of the double root"
            );
        }
        let events = structure_events(&[wall], &ivl(0, 2), &Q::new(1, 1 << 20))
            .expect("the chain must verify");
        assert!(
            events.iter().any(|e| {
                e.kinds.contains(&EventKind::Tangent(0))
                    && e.at.lo.cmp(&Q::from_i128(1)) != core::cmp::Ordering::Greater
                    && Q::from_i128(1).cmp(&e.at.hi) != core::cmp::Ordering::Greater
            }),
            "the double tangency at σ = 1 must be bracketed"
        );
    }

    /// **Walls that never interact produce no events at all.** Two parallel affine walls (`µ = 1`,
    /// `µ = 2`) have constant discriminants, identically-zero leading coefficients and a constant
    /// resultant, so the partition stays empty and the tracer sweeps the window in one cell.
    #[test]
    fn parallel_walls_leave_the_window_unpartitioned() {
        let events = structure_events(
            &[form(0, 1, -1), form(0, 1, -2)],
            &ivl(-3, 3),
            &Q::new(1, 1 << 20),
        )
        .expect("the chains must verify");
        assert!(
            events.is_empty(),
            "expected no events, got {:?}",
            events.iter().map(|e| &e.kinds).collect::<Vec<_>>()
        );
    }

    /// **The event set finds the corners the band builder bisects for.** On the AUTH.1e.4 square
    /// prism, scan the ruling patch across the footprint and note every σ-cell in which the wall
    /// governing an end changes — 1e.4's `CORNER_SWEEPS` searches for exactly these. Each one must
    /// contain an event bracket, and the bracket names the *same two walls* the patch changed
    /// between.
    #[test]
    fn every_governing_wall_change_on_the_square_prism_has_an_event() {
        let chart = fixtures::devices::cone_wrap();
        let cfg = DevConfig::tight();
        let (cx, cy, h) = (Q::new(-1, 2), Q::new(27, 10), Q::new(1, 8));
        let profile = arrange2d::profile::Profile::new()
            .rect(cx.clone(), cy.clone(), h.clone(), h.clone())
            .into_edges();
        let cast = crate::extrude::Cast::new(
            xy_frame(),
            crate::extrude::Apex::direction([Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)])
                .expect("a real direction"),
        )
        .expect("the apex is off the frame plane");
        let walls = cast.carrier_walls(&profile).expect("four distinct lines");
        let zero = Q::from_i128(0);
        let forms: Vec<MuCut<Bignum>> = walls
            .iter()
            .map(|w| cut_mu_form(&chart, w, &zero).expect("each wall pulls back"))
            .collect();
        let window = bounding_window(
            &chart,
            &CutSurface::Cylinder {
                axis_point: [cx.clone(), cy.clone(), Q::from_i128(0)],
                axis_dir: [Q::from_i128(0), Q::from_i128(0), Q::from_i128(1)],
                r2: h.mul(&h).mul(&Q::from_i128(2)),
            },
        );
        let inside = |s: &Q, mu: &Q| -> Option<bool> {
            let p = chart.surface(mu, &zero).eval(s)?;
            cast.contains(&p, &profile)
        };
        let events =
            structure_events(&forms, &window, &Q::new(1, 1 << 24)).expect("the chains must verify");

        // Scan as 1e.4 does, and record each cell in which a governing wall changed.
        const SCAN: i128 = 400;
        let at = |k: i128| {
            window
                .lo
                .add(&window.hi.sub(&window.lo).mul(&Q::new(k, SCAN)))
        };
        let mut prev: Option<(Q, WallRoot, WallRoot)> = None;
        let mut changes = 0;
        for k in 0..=SCAN {
            let s = at(k);
            let p = match ruling_patches(&forms, &s, &inside, &cfg.sqrt_eps) {
                // A convex profile meets each ruling in one stretch; anything else here would be
                // this fixture changing shape, not the event set failing.
                Ok(ps) if ps.len() == 1 => ps.into_iter().next().expect("one stretch"),
                _ => {
                    prev = None;
                    continue;
                }
            };
            if let Some((ps, plo, phi)) = &prev {
                for (a, b) in [(*plo, p.lo_at), (*phi, p.hi_at)] {
                    if a.0 == b.0 {
                        continue;
                    }
                    changes += 1;
                    let (lo, hi) = (a.0.min(b.0), a.0.max(b.0));
                    assert!(
                        events.iter().any(|e| {
                            e.kinds.contains(&EventKind::Meet(lo, hi))
                                && e.at.lo.cmp(&s) != core::cmp::Ordering::Greater
                                && ps.cmp(&e.at.hi) != core::cmp::Ordering::Greater
                        }),
                        "the corner between walls {lo} and {hi} in σ ∈ [{}, {}] has no event",
                        to_f64(ps),
                        to_f64(&s)
                    );
                }
            }
            prev = Some((s, p.lo_at, p.hi_at));
        }
        // Two, and the geometry says two: a ruling family sweeping across a convex quadrilateral
        // switches the edge it *enters* by once and the edge it *leaves* by once. (Guessing four
        // here — one per side — was wrong, and the assertion earned its keep by saying so.)
        assert!(
            changes >= 2,
            "the square's band must change governing wall on both branches, saw {changes}"
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
