//! **The three arc constructions** — and the one place an import can cost anything.
//!
//! `Profile::arc` needs its endpoints to satisfy `(x − cx)² + (y − cy)² = r²` **exactly**. It is
//! total, so an inconsistent arc is emitted *as drawn* and the arrangement downstream receives bad
//! data rather than earning a refusal — which is why every arc built here is checked against its
//! own circle by exact rational equality before it leaves ([`ExactArc::is_consistent`]), and why
//! that check is the milestone's runtime-checked hypothesis rather than a paragraph in a design
//! document.
//!
//! A file can state an arc in three ways, and the interesting fact is that they are **not** equally
//! expensive. The over-determination differs, so a different datum has to move:
//!
//! | source | what the file fixes | what moves | `δ` |
//! |---|---|---|---|
//! | DXF `LWPOLYLINE` bulge ([`from_bulge`]) | two endpoints + `tan(Δθ/4)` | *nothing* | `0` |
//! | SVG `A` ([`from_endpoints_radius`]) | two endpoints + radius | the **centre**, along the endpoints' rational bisector | a radius deviation |
//! | DXF `ARC` ([`from_centre_angles`]) | centre + radius + two angles | the **endpoints**, around the exact circle | an endpoint distance |
//!
//! Only the last is genuinely lossy, and its `δ` is certified by the same
//! [`develop::interval`](develop::interval) enclosures the development runs on — not estimated in
//! floating point. So: **export your outline with bulges and the import is exact; export the same
//! outline as `ARC` entities and it costs a certified δ.**

use develop::interval::{RatIv, cos_on, pi, sin_on};
use lattice::{Backend, Rat};

/// Why an arc could not be built. Every variant is a **refusal** — the file does not describe a
/// circular arc we can carry exactly — never a silently repaired shape.
#[derive(Debug)]
pub enum ArcFault<B: Backend> {
    /// The two endpoints coincide, so there is no chord and no arc.
    DegenerateChord,
    /// A bulge of zero is a straight segment, not an arc — the caller emits a line.
    StraightBulge,
    /// A non-positive radius.
    NonPositiveRadius,
    /// The construction's certified backward error exceeds the caller's budget.
    ToleranceExceeded {
        /// The certified backward error actually achieved.
        delta: Rat<B>,
        /// The budget it had to meet.
        budget: Rat<B>,
    },
    /// The runtime-checked hypothesis fired: a constructed endpoint is not on its own circle. This
    /// is a bug in this module, surfaced rather than shipped — see the module docs.
    NotOnCircle,
}

impl<B: Backend> ArcFault<B> {
    /// The variant's name, for a refusal message that must not carry the backend type parameter.
    pub fn name(&self) -> &'static str {
        match self {
            ArcFault::DegenerateChord => "DegenerateChord",
            ArcFault::StraightBulge => "StraightBulge",
            ArcFault::NonPositiveRadius => "NonPositiveRadius",
            ArcFault::ToleranceExceeded { .. } => "ToleranceExceeded",
            ArcFault::NotOnCircle => "NotOnCircle",
        }
    }
}

/// How hard to work, and how much backward error to accept.
///
/// The [five-part contract](../../../docs/construction-api-design.md) in one type: approximation is
/// **explicit** (you pass this), **opt-in** (bulge arcs never consult it), **controllable**
/// (`budget`), **certified** (`δ` comes from interval enclosures) and **refineable** (`iters` drives
/// `δ` down one bit per step — to a stated floor, not to zero; see [`with_refinement`](Self::with_refinement)).
pub struct ArcTolerance<B: Backend> {
    budget: Option<Rat<B>>,
    iters: usize,
    terms: usize,
}

impl<B: Backend> Clone for ArcTolerance<B> {
    fn clone(&self) -> Self {
        ArcTolerance {
            budget: self.budget.clone(),
            iters: self.iters,
            terms: self.terms,
        }
    }
}

impl<B: Backend> Default for ArcTolerance<B> {
    fn default() -> Self {
        Self::report_only()
    }
}

impl<B: Backend> ArcTolerance<B> {
    /// Accept whatever backward error the construction achieves, and report it. For diagnostics
    /// and for callers that gate on the assembled report rather than per entity.
    pub fn report_only() -> Self {
        ArcTolerance {
            budget: None,
            iters: 64,
            terms: 24,
        }
    }

    /// Refuse any arc whose certified backward error exceeds `budget` (in the target unit).
    pub fn within(budget: Rat<B>) -> Self {
        ArcTolerance {
            budget: Some(budget),
            iters: 64,
            terms: 24,
        }
    }

    /// Refinement depth — bisection steps for a DXF `ARC`, Newton steps for an SVG `A`.
    ///
    /// `δ` shrinks geometrically, one bit per step (measured: `1.3·10⁻¹` at 4, `3.7·10⁻⁵` at 16,
    /// `2.0·10⁻¹⁰` at 32), until it reaches the **enclosure floor** at roughly `1.6·10⁻¹⁶·r` — the
    /// accumulated outward rounding of [`develop::interval`]'s own `ROUND_BITS`, not a limit of the
    /// search. Past that, more steps buy nothing and more series `terms` very slightly *widen* the
    /// bound, because each extra term is another rounding. The default sits at the floor.
    ///
    /// Refinement converging to a floor rather than to zero is the sanctioned shape here (the same
    /// one `docs/construction-api-design.md` records for a strain budget); the floor is stated so
    /// that it is a known quantity rather than a surprise at the fourth decimal place.
    pub fn with_refinement(mut self, iters: usize) -> Self {
        self.iters = iters;
        self
    }

    /// Series terms for the certified `cos`/`sin`/`π` enclosures.
    pub fn with_terms(mut self, terms: usize) -> Self {
        self.terms = terms;
        self
    }

    /// Refuse when `delta` is over budget; pass it through otherwise.
    ///
    /// Public because the budget has to be honoured **after** assembly too: a junction re-gauge
    /// ([`ExactArc::regauged`]) raises an arc's `δ` once its neighbours are known, which is past
    /// every per-entity check.
    pub fn check(&self, delta: Rat<B>) -> Result<Rat<B>, ArcFault<B>> {
        match &self.budget {
            Some(b) if delta > *b => Err(ArcFault::ToleranceExceeded {
                delta,
                budget: b.clone(),
            }),
            _ => Ok(delta),
        }
    }
}

/// A circular arc whose endpoints lie **exactly** on its own circle, with the certified backward
/// error of getting there.
#[derive(Debug)]
pub struct ExactArc<B: Backend> {
    /// Circle centre x.
    pub cx: Rat<B>,
    /// Circle centre y.
    pub cy: Rat<B>,
    /// Squared radius. Exact, and not necessarily a rational square — which is fine, since the
    /// arrangement stores circles by `r²` for exactly this reason.
    pub r2: Rat<B>,
    /// Start point, exactly on the circle.
    pub start: [Rat<B>; 2],
    /// End point, exactly on the circle.
    pub end: [Rat<B>; 2],
    /// Whether the arc runs counter-clockwise from `start` to `end`.
    pub ccw: bool,
    /// The certified backward error: how far this arc is from the one the file stated. Exactly
    /// zero for [`from_bulge`].
    pub delta: Rat<B>,
}

impl<B: Backend> Clone for ExactArc<B> {
    fn clone(&self) -> Self {
        ExactArc {
            cx: self.cx.clone(),
            cy: self.cy.clone(),
            r2: self.r2.clone(),
            start: [self.start[0].clone(), self.start[1].clone()],
            end: [self.end[0].clone(), self.end[1].clone()],
            ccw: self.ccw,
            delta: self.delta.clone(),
        }
    }
}

impl<B: Backend> ExactArc<B> {
    /// **The runtime-checked hypothesis.** Both endpoints satisfy `(x − cx)² + (y − cy)² = r²` as
    /// an exact rational equality — no tolerance, because there is nothing here to be tolerant of.
    ///
    /// Every constructor in this module runs it before returning, so a caller never has to. It is
    /// public because a translator that acquires a fourth arc source should be held to it too.
    pub fn is_consistent(&self) -> bool {
        self.on_circle(&self.start) && self.on_circle(&self.end)
    }

    fn on_circle(&self, p: &[Rat<B>; 2]) -> bool {
        let dx = p[0].sub(&self.cx);
        let dy = p[1].sub(&self.cy);
        dx.mul(&dx).add(&dy.mul(&dy)) == self.r2
    }

    /// An arc whose data is **already** exactly consistent — `δ = 0` by assertion, not by hope.
    ///
    /// The constructor for geometry that needs no search because the file states it in a form the
    /// rationals can hold outright: a rounded rectangle's corner, whose endpoints are the axis-
    /// aligned tangent points `(cx ± r, cy)` and `(cx, cy ± r)`. Refuses [`ArcFault::NotOnCircle`]
    /// if the caller is wrong about that, which is the point of routing through here rather than
    /// building the struct literal.
    pub fn exact(
        cx: Rat<B>,
        cy: Rat<B>,
        r2: Rat<B>,
        start: [Rat<B>; 2],
        end: [Rat<B>; 2],
        ccw: bool,
    ) -> Result<Self, ArcFault<B>> {
        ExactArc {
            cx,
            cy,
            r2,
            start,
            end,
            ccw,
            delta: Rat::from_i128(0),
        }
        .sealed()
    }

    /// **The junction lift** — re-gauge the arc onto a vertex a neighbour already owns, keeping its
    /// centre and its own sweep, and paying for it in `δ`.
    ///
    /// Where two `ARC` entities meet, neither side is free: their reconstructed endpoints sit on
    /// *different* circles and neither may simply be moved, because an arc's endpoints belong to
    /// its circle. What *is* free is the radius. So this holds the centre, takes `start` as given —
    /// which fixes `r² := |start − c|²` — and carries the far end round by the arc's **own** sweep,
    /// applied as the exact rational rotation that the file's two endpoints already encode:
    ///
    /// ```text
    /// cos Δθ = (u·w)/r²      sin Δθ = (u×w)/r²      u = start − c,  w = end − c
    /// ```
    ///
    /// Both are rational, and `cos² + sin² = |u|²|w|²/r⁴ = 1` **exactly**, because
    /// [`is_consistent`](Self::is_consistent) has already put `u` and `w` on the same circle. A
    /// rotation preserves length, so the new end lands on the new circle exactly — the arc is
    /// re-gauged, never bent.
    ///
    /// `δ` grows by the distance the start moved (bounded `|Δx| + |Δy|`, no square root needed):
    /// the rotation is an isometry, so the far end moves by exactly that much too, and both
    /// endpoints stay within `δ_old + |Δ|` of the ones the file stated.
    ///
    /// ```
    /// use interchange::arc::from_centre_angles;
    /// use lattice::{Bignum, Rat};
    /// type Q = Rat<Bignum>;
    ///
    /// let tol = Default::default();
    /// // A quarter turn of the unit circle, then re-gauged onto a vertex a hair further out.
    /// let arc = from_centre_angles::<Bignum>(
    ///     [Q::from_i128(0), Q::from_i128(0)], &Q::from_i128(1),
    ///     &Q::from_i128(0), &Q::from_i128(90), &tol,
    /// ).expect("a quarter arc");
    /// let moved = arc.regauged([Q::new(1001, 1000), Q::from_i128(0)]).expect("re-gauged");
    ///
    /// assert!(moved.is_consistent());                       // still exactly on its own circle
    /// assert_eq!(moved.r2, Q::new(1_002_001, 1_000_000));   // the radius is what moved
    /// assert_eq!(moved.end, [Q::from_i128(0), Q::new(1001, 1000)]); // the sweep is unchanged
    /// ```
    pub fn regauged(&self, start: [Rat<B>; 2]) -> Result<Self, ArcFault<B>> {
        if self.r2.sign() <= 0 {
            return Err(ArcFault::NonPositiveRadius);
        }
        let u = [self.start[0].sub(&self.cx), self.start[1].sub(&self.cy)];
        let w = [self.end[0].sub(&self.cx), self.end[1].sub(&self.cy)];
        let cos = u[0].mul(&w[0]).add(&u[1].mul(&w[1])).div(&self.r2);
        let sin = u[0].mul(&w[1]).sub(&u[1].mul(&w[0])).div(&self.r2);

        let v = [start[0].sub(&self.cx), start[1].sub(&self.cy)];
        let end = [
            self.cx.add(&cos.mul(&v[0]).sub(&sin.mul(&v[1]))),
            self.cy.add(&sin.mul(&v[0]).add(&cos.mul(&v[1]))),
        ];
        let moved = abs(&start[0].sub(&self.start[0])).add(&abs(&start[1].sub(&self.start[1])));

        ExactArc {
            cx: self.cx.clone(),
            cy: self.cy.clone(),
            r2: norm2(&v),
            start,
            end,
            ccw: self.ccw,
            delta: self.delta.add(&moved),
        }
        .sealed()
    }

    /// Check the hypothesis, then hand the arc back. The one exit every constructor uses.
    fn sealed(self) -> Result<Self, ArcFault<B>> {
        if self.is_consistent() {
            Ok(self)
        } else {
            Err(ArcFault::NotOnCircle)
        }
    }
}

/// `|v|²` for a 2-vector.
fn norm2<B: Backend>(v: &[Rat<B>; 2]) -> Rat<B> {
    v[0].mul(&v[0]).add(&v[1].mul(&v[1]))
}

/// `|q|`, exactly.
fn abs<B: Backend>(q: &Rat<B>) -> Rat<B> {
    if q.sign() < 0 { q.neg() } else { q.clone() }
}

// --- (1) the free one: a DXF bulge ---------------------------------------------------------

/// **DXF `LWPOLYLINE`/`VERTEX` bulge — exact, `δ = 0`.**
///
/// The bulge is `b = tan(Δθ/4)`, a rational straight out of the file, and both vertices are
/// rationals. With `d = p₁ − p₀` and `n = perp(d)` (*unnormalized*, so rational), the centre is
///
/// ```text
/// c = (p₀ + p₁)/2 + λ·n        λ = (1 − b²) / (4b)
/// ```
///
/// which is rational because `cot(Δθ/2) = (1 − b²)/(2b)` and the normalization of `n` cancels
/// against the half-chord length. Setting `r² := |p₀ − c|²` then puts **both** endpoints exactly on
/// the circle — `p₁` because `c` sits on their perpendicular bisector by construction, which is
/// what `d · n = 0` says.
///
/// ```
/// use interchange::arc::from_bulge;
/// use lattice::{Bignum, Rat};
/// type Q = Rat<Bignum>;
///
/// // A quarter turn: b = tan(22.5°) = √2 − 1. Take the *rational* b = 1/2 instead — any rational
/// // bulge is a legitimate arc, which is the point.
/// let arc = from_bulge::<Bignum>(
///     [Q::from_i128(1), Q::from_i128(0)],
///     [Q::from_i128(0), Q::from_i128(1)],
///     &Q::new(1, 2),
/// ).expect("a bulge is always an exact arc");
/// assert_eq!(arc.delta, Q::from_i128(0));
/// assert!(arc.is_consistent());
/// ```
pub fn from_bulge<B: Backend>(
    p0: [Rat<B>; 2],
    p1: [Rat<B>; 2],
    bulge: &Rat<B>,
) -> Result<ExactArc<B>, ArcFault<B>> {
    if bulge.is_zero() {
        return Err(ArcFault::StraightBulge);
    }
    let d = [p1[0].sub(&p0[0]), p1[1].sub(&p0[1])];
    if d[0].is_zero() && d[1].is_zero() {
        return Err(ArcFault::DegenerateChord);
    }
    let n = [d[1].neg(), d[0].clone()];
    let half = Rat::new(1, 2);
    let mid = [p0[0].add(&p1[0]).mul(&half), p0[1].add(&p1[1]).mul(&half)];

    let one = Rat::from_i128(1);
    let lambda = one
        .sub(&bulge.mul(bulge))
        .div(&bulge.mul(&Rat::from_i128(4)));
    let cx = mid[0].add(&lambda.mul(&n[0]));
    let cy = mid[1].add(&lambda.mul(&n[1]));
    let r2 = norm2(&[p0[0].sub(&cx), p0[1].sub(&cy)]);

    ExactArc {
        cx,
        cy,
        r2,
        start: p0,
        end: p1,
        ccw: bulge.sign() > 0,
        delta: Rat::from_i128(0),
    }
    .sealed()
}

// --- (2) the cheap one: an SVG endpoint arc ------------------------------------------------

/// **SVG `A` — hold the endpoints, move the centre; `δ` is a *radius* deviation.**
///
/// SVG states the two endpoints and a radius, so the centre is derived. The centre must lie on the
/// endpoints' perpendicular bisector — a **rational line** — and *any* rational point on it makes
/// `r² := |p₀ − c|²` satisfy `|p₁ − c|² = r²` identically. So both endpoints land exactly on the
/// circle and the only thing that can be wrong is the radius.
///
/// Reporting the deviation there rather than at an endpoint is also the honest reading of the
/// format: SVG's own specification already permits scaling the radius up when the stated one cannot
/// span the endpoints, and this does exactly that (with `δ` saying how far it went).
///
/// The centre offset is `t = √(r²/|d|² − 1/4)`, found by rational Newton from above — no floating
/// point anywhere on this path, and `iters` drives `δ` monotonically to zero.
pub fn from_endpoints_radius<B: Backend>(
    p0: [Rat<B>; 2],
    p1: [Rat<B>; 2],
    r: &Rat<B>,
    large_arc: bool,
    sweep: bool,
    tol: &ArcTolerance<B>,
) -> Result<ExactArc<B>, ArcFault<B>> {
    if r.sign() <= 0 {
        return Err(ArcFault::NonPositiveRadius);
    }
    let d = [p1[0].sub(&p0[0]), p1[1].sub(&p0[1])];
    let dd = norm2(&d);
    if dd.is_zero() {
        return Err(ArcFault::DegenerateChord);
    }
    let n = [d[1].neg(), d[0].clone()];
    let half = Rat::new(1, 2);
    let mid = [p0[0].add(&p1[0]).mul(&half), p0[1].add(&p1[1]).mul(&half)];

    // t² = r²/|d|² − 1/4. Negative means the chord is longer than the diameter, which SVG resolves
    // by growing the radius — here, t = 0 and the centre at the midpoint.
    let rr = r.mul(r);
    let k = rr.div(&dd).sub(&Rat::new(1, 4));
    let t = if k.sign() <= 0 {
        Rat::from_i128(0)
    } else {
        crate::num::sqrt_rational(&k, tol.iters)
    };

    // SVG F.6.5: the centre sits on the `+perp` side exactly when the two flags differ.
    let signed = if large_arc == sweep { t.neg() } else { t };
    let cx = mid[0].add(&signed.mul(&n[0]));
    let cy = mid[1].add(&signed.mul(&n[1]));
    let r2 = norm2(&[p0[0].sub(&cx), p0[1].sub(&cy)]);

    // |r' − r| = |r'² − r²| / (r' + r) ≤ |r'² − r²| / r, since r' > 0. All rational.
    let delta = tol.check(abs(&r2.sub(&rr)).div(r))?;

    ExactArc {
        cx,
        cy,
        r2,
        start: p0,
        end: p1,
        ccw: sweep,
        delta,
    }
    .sealed()
}

// --- (3) the certified one: a DXF ARC ------------------------------------------------------

/// **DXF `ARC` — hold the circle, move the endpoints; `δ` is certified.**
///
/// The file gives centre, radius *and* two angles: four exact rationals describing a point that is
/// irrational. The circle itself stays exact, and each endpoint moves onto it through the rational
/// tangent-half-angle rotation
///
/// ```text
/// P(t) = c + r·( (1 − t²)/(1 + t²) ,  2t/(1 + t²) )
/// ```
///
/// which is exactly on the circle for **every** rational `t`, because `(1−t²)² + (2t)² = (1+t²)²`.
/// The angle is first reduced by exact quarter turns (a quarter turn is `(x, y) ↦ (−y, x)`, free),
/// leaving a residual in `[0°, 90°)` where `t ∈ [0, 1)` — so the search is a bisection on a fixed
/// bracket rather than a float `tan`, and no floating point appears on this path either.
///
/// `δ` is the distance from the emitted point to the true one, bounded through the certified
/// [`cos_on`]/[`sin_on`]/[`pi`] enclosures. Angles are in **degrees**, as DXF writes them.
pub fn from_centre_angles<B: Backend>(
    centre: [Rat<B>; 2],
    r: &Rat<B>,
    deg_start: &Rat<B>,
    deg_end: &Rat<B>,
    tol: &ArcTolerance<B>,
) -> Result<ExactArc<B>, ArcFault<B>> {
    if r.sign() <= 0 {
        return Err(ArcFault::NonPositiveRadius);
    }
    let (s, ds) = point_at_degrees(&centre, r, deg_start, tol);
    let (e, de) = point_at_degrees(&centre, r, deg_end, tol);
    let delta = tol.check(if ds > de { ds } else { de })?;

    ExactArc {
        cx: centre[0].clone(),
        cy: centre[1].clone(),
        r2: r.mul(r),
        start: s,
        end: e,
        // DXF arcs are always counter-clockwise from the start angle to the end angle.
        ccw: true,
        delta,
    }
    .sealed()
}

/// The rational point of the circle `(centre, r)` nearest the true `deg`-degree point, and a
/// certified bound on how far away that is.
fn point_at_degrees<B: Backend>(
    centre: &[Rat<B>; 2],
    r: &Rat<B>,
    deg: &Rat<B>,
    tol: &ArcTolerance<B>,
) -> ([Rat<B>; 2], Rat<B>) {
    let ninety = Rat::from_i128(90);
    // Reduce into [0°, 360°) then split off whole quarter turns: both steps are exact rational
    // arithmetic, and a quarter turn costs nothing to apply.
    let full = Rat::from_i128(360);
    let wrapped = deg.sub(&full.mul(&deg.div(&full).floor()));
    let quarters = wrapped.div(&ninety).floor();
    let rem = wrapped.sub(&ninety.mul(&quarters));

    // An angle that is a whole multiple of 90° has already been solved by the reduction: the point
    // is `c` plus a quarter turn of `(r, 0)`, with nothing left to approximate. This is not a
    // micro-optimization — axis-aligned quarter arcs are what a rounded rectangle is made of, and
    // they deserve to import at `δ = 0` rather than at the enclosure floor.
    let (g, h, delta) = if rem.is_zero() {
        (Rat::from_i128(1), Rat::from_i128(0), Rat::from_i128(0))
    } else {
        // The residual angle in radians, as a certified enclosure.
        let rad = pi::<B>(tol.terms).scale(&rem.div(&Rat::from_i128(180)));
        let cos_iv = cos_on(&rad, tol.terms);
        let sin_iv = sin_on(&rad, tol.terms);

        let t = solve_half_tangent(&cos_iv, tol.iters);
        let (g, h) = half_tangent_point(&t);

        // Distance from the emitted direction to the enclosed true one: |Δ| ≤ |Δx| + |Δy|, each
        // bounded by the wider side of its enclosure. Scaled by r, this is the displacement.
        let ex = farthest_from(&g, &cos_iv);
        let ey = farthest_from(&h, &sin_iv);
        let delta = r.mul(&ex.add(&ey));
        (g, h, delta)
    };

    // Apply the whole quarter turns exactly: (x, y) ↦ (−y, x), `quarters` times.
    let (mut px, mut py) = (r.mul(&g), r.mul(&h));
    let turns = quarter_turns(&quarters);
    for _ in 0..turns {
        let nx = py.neg();
        py = px;
        px = nx;
    }
    ([centre[0].add(&px), centre[1].add(&py)], delta)
}

/// `quarters` as a small turn count in `0..4` (it is an exact integer-valued rational in that range
/// by construction, so this is a read, not a conversion with a decision in it).
fn quarter_turns<B: Backend>(quarters: &Rat<B>) -> usize {
    let mut n = 0usize;
    let one = Rat::from_i128(1);
    let mut acc = Rat::from_i128(0);
    while acc < *quarters && n < 4 {
        acc = acc.add(&one);
        n += 1;
    }
    n
}

/// `((1 − t²)/(1 + t²), 2t/(1 + t²))` — a rational point of the **unit** circle, exactly.
fn half_tangent_point<B: Backend>(t: &Rat<B>) -> (Rat<B>, Rat<B>) {
    let t2 = t.mul(t);
    let one = Rat::from_i128(1);
    let den = one.add(&t2);
    (one.sub(&t2).div(&den), t.mul(&Rat::from_i128(2)).div(&den))
}

/// Bisect `t ∈ [0, 1]` until `(1 − t²)/(1 + t²)` lands inside the enclosure of `cos θ`.
///
/// `g(t) = (1 − t²)/(1 + t²)` is `cos(2·arctan t)`, strictly decreasing from `1` to `0` on `[0, 1]`
/// — so the comparison is monotone and each step is an exact rational test against the enclosure's
/// two ends. It stops early once `g(m)` is inside the enclosure, because no further bisection can
/// distinguish the candidates the certificate itself cannot.
fn solve_half_tangent<B: Backend>(cos_iv: &RatIv<B>, iters: usize) -> Rat<B> {
    let half = Rat::new(1, 2);
    let (mut lo, mut hi) = (Rat::from_i128(0), Rat::from_i128(1));
    let mut m = lo.add(&hi).mul(&half);
    for _ in 0..iters.max(1) {
        m = lo.add(&hi).mul(&half);
        let (g, _) = half_tangent_point(&m);
        if g > *cos_iv.hi() {
            // g(m) is above every possible cos θ, and g decreases — so the true t is past m.
            lo = m.clone();
        } else if g < *cos_iv.lo() {
            hi = m.clone();
        } else {
            break;
        }
    }
    m
}

/// How far `x` can be from a value enclosed by `iv` — the wider of the two ends.
fn farthest_from<B: Backend>(x: &Rat<B>, iv: &RatIv<B>) -> Rat<B> {
    let a = abs(&x.sub(iv.lo()));
    let b = abs(&x.sub(iv.hi()));
    if a > b { a } else { b }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    fn q(n: i128, d: i128) -> Q {
        Q::new(n, d)
    }

    /// A float view of a rational, for readable assertions only — never a predicate.
    fn f(r: &Q) -> f64 {
        let (n, d) = r.numer_denom_decimal();
        n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
    }

    /// The headline: a bulge arc costs **nothing**, and the endpoints are the file's own — not
    /// merely close to them.
    #[test]
    fn a_bulge_arc_is_exact_and_moves_nothing() {
        let p0 = [Q::from_i128(1), Q::from_i128(0)];
        let p1 = [Q::from_i128(0), Q::from_i128(1)];
        let arc = from_bulge::<Bignum>(p0.clone(), p1.clone(), &q(1, 2)).expect("exact");
        assert_eq!(
            arc.delta,
            Q::from_i128(0),
            "a bulge costs no backward error"
        );
        assert_eq!(arc.start, p0, "the file's own start point, unmoved");
        assert_eq!(arc.end, p1, "the file's own end point, unmoved");
        assert!(arc.is_consistent());
    }

    /// The classical case, worked by hand: a bulge of `tan(22.5°)` is a quarter turn. Its rational
    /// cousin `b = 1/2` is a different arc but the same construction, and `b = 1` — a semicircle —
    /// must put the centre exactly at the chord midpoint.
    #[test]
    fn the_bulge_centre_is_the_one_the_geometry_predicts() {
        // b = 1 ⇒ λ = 0 ⇒ centre at the midpoint (a semicircle).
        let arc = from_bulge::<Bignum>(
            [Q::from_i128(-1), Q::from_i128(0)],
            [Q::from_i128(1), Q::from_i128(0)],
            &Q::from_i128(1),
        )
        .expect("semicircle");
        assert_eq!(arc.cx, Q::from_i128(0));
        assert_eq!(arc.cy, Q::from_i128(0));
        assert_eq!(arc.r2, Q::from_i128(1));

        // A negative bulge is the same circle mirrored across the chord, and runs clockwise.
        let cw = from_bulge::<Bignum>(
            [Q::from_i128(1), Q::from_i128(0)],
            [Q::from_i128(0), Q::from_i128(1)],
            &q(-1, 2),
        )
        .expect("clockwise");
        let ccw = from_bulge::<Bignum>(
            [Q::from_i128(1), Q::from_i128(0)],
            [Q::from_i128(0), Q::from_i128(1)],
            &q(1, 2),
        )
        .expect("counter-clockwise");
        assert!(!cw.ccw && ccw.ccw);
        // The two centres are reflections of each other in the chord's midpoint line.
        assert_eq!(cw.cx.add(&ccw.cx), Q::from_i128(1));
        assert_eq!(cw.cy.add(&ccw.cy), Q::from_i128(1));
    }

    /// **Which datum moved.** For an SVG arc it is the radius, and the endpoints are untouched —
    /// a magnitude-only check would pass a translator that moved an endpoint instead.
    #[test]
    fn an_svg_arc_moves_the_radius_and_never_an_endpoint() {
        let p0 = [Q::from_i128(1), Q::from_i128(0)];
        let p1 = [Q::from_i128(0), Q::from_i128(1)];
        let arc = from_endpoints_radius::<Bignum>(
            p0.clone(),
            p1.clone(),
            &Q::from_i128(1),
            false,
            true,
            &ArcTolerance::report_only(),
        )
        .expect("an endpoint arc");
        assert_eq!(arc.start, p0, "the endpoints are the file's, exactly");
        assert_eq!(arc.end, p1);
        assert!(arc.is_consistent());
        // r = 1 with these endpoints has centre (0,0) exactly — t = √(1/2 − 1/4) = 1/2 is
        // rational here, so this particular arc costs nothing.
        assert_eq!(arc.cx, Q::from_i128(0));
        assert_eq!(arc.cy, Q::from_i128(0));
        assert_eq!(arc.delta, Q::from_i128(0), "√(1/4) is rational");
    }

    /// The flags pick between the two centres, and getting that backwards is a silently wrong part.
    #[test]
    fn the_svg_flags_choose_between_the_two_centres() {
        let p0 = [Q::from_i128(1), Q::from_i128(0)];
        let p1 = [Q::from_i128(0), Q::from_i128(1)];
        let tol = ArcTolerance::report_only();
        let small = from_endpoints_radius::<Bignum>(
            p0.clone(),
            p1.clone(),
            &Q::from_i128(1),
            false,
            true,
            &tol,
        )
        .expect("small CCW");
        let large = from_endpoints_radius::<Bignum>(p0, p1, &Q::from_i128(1), true, true, &tol)
            .expect("large CCW");
        assert_eq!([small.cx, small.cy], [Q::from_i128(0), Q::from_i128(0)]);
        assert_eq!([large.cx, large.cy], [Q::from_i128(1), Q::from_i128(1)]);
    }

    /// A chord longer than the diameter: SVG grows the radius, and `δ` says by how much rather
    /// than the reader pretending it did not happen.
    #[test]
    fn an_unspannable_radius_grows_and_reports_it() {
        let arc = from_endpoints_radius::<Bignum>(
            [Q::from_i128(0), Q::from_i128(0)],
            [Q::from_i128(10), Q::from_i128(0)],
            &Q::from_i128(1),
            false,
            true,
            &ArcTolerance::report_only(),
        )
        .expect("radius grown per the SVG spec");
        assert_eq!(arc.r2, Q::from_i128(25), "the semicircle on the chord");
        assert!(arc.delta.sign() > 0, "and it is reported");
        assert!(arc.is_consistent());
        // Over a tight budget the same arc is refused by name instead.
        let refused = from_endpoints_radius::<Bignum>(
            [Q::from_i128(0), Q::from_i128(0)],
            [Q::from_i128(10), Q::from_i128(0)],
            &Q::from_i128(1),
            false,
            true,
            &ArcTolerance::within(q(1, 1000)),
        );
        assert!(matches!(refused, Err(ArcFault::ToleranceExceeded { .. })));
    }

    /// **Which datum moved**, the other way: a DXF `ARC` keeps the circle and moves the endpoints.
    /// The certified `δ` is tiny, and the endpoints are *exactly* on the authored circle.
    #[test]
    fn a_dxf_arc_keeps_the_circle_and_certifies_the_endpoints() {
        // A quarter arc of radius 5 about (2, 3), from 30° to 120°.
        let arc = from_centre_angles::<Bignum>(
            [Q::from_i128(2), Q::from_i128(3)],
            &Q::from_i128(5),
            &Q::from_i128(30),
            &Q::from_i128(120),
            &ArcTolerance::report_only(),
        )
        .expect("a certified arc");
        assert_eq!(arc.cx, Q::from_i128(2), "the centre is the file's");
        assert_eq!(arc.cy, Q::from_i128(3));
        assert_eq!(arc.r2, Q::from_i128(25), "and so is the radius");
        assert!(arc.is_consistent(), "endpoints exactly on that circle");
        assert!(
            arc.delta < q(1, 1_000_000_000),
            "delta {} is not sub-nanometre",
            f(&arc.delta)
        );
        // The float positions are where 30°/120° actually are.
        let (sx, sy) = (f(&arc.start[0]), f(&arc.start[1]));
        assert!((sx - (2.0 + 5.0 * 30f64.to_radians().cos())).abs() < 1e-9);
        assert!((sy - (3.0 + 5.0 * 30f64.to_radians().sin())).abs() < 1e-9);
        let (ex, ey) = (f(&arc.end[0]), f(&arc.end[1]));
        assert!((ex - (2.0 + 5.0 * 120f64.to_radians().cos())).abs() < 1e-9);
        assert!((ey - (3.0 + 5.0 * 120f64.to_radians().sin())).abs() < 1e-9);
    }

    /// Every quadrant, including the ones the quarter-turn reduction has to rotate — and the
    /// angles a real file writes without thinking (negative, past 360°).
    #[test]
    fn every_quadrant_and_every_wrapping_lands_on_the_circle() {
        let tol = ArcTolerance::<Bignum>::report_only();
        for deg in [
            0i32, 45, 90, 135, 180, 225, 270, 315, 359, 360, 405, -30, -270, 720,
        ] {
            let arc = from_centre_angles::<Bignum>(
                [Q::from_i128(0), Q::from_i128(0)],
                &Q::from_i128(1),
                &Q::from_i128(deg.into()),
                &Q::from_i128((deg + 10).into()),
                &tol,
            )
            .unwrap_or_else(|e| panic!("{deg}°: {e:?}"));
            assert!(arc.is_consistent(), "{deg}° is not on its circle");
            let (x, y) = (f(&arc.start[0]), f(&arc.start[1]));
            let want = f64::from(deg).to_radians();
            assert!(
                (x - want.cos()).abs() < 1e-9 && (y - want.sin()).abs() < 1e-9,
                "{deg}° landed at ({x}, {y})"
            );
        }
    }

    /// **Refineable, and where it stops** — clause 5 of the five-part contract, stated honestly.
    ///
    /// `δ` must shrink with work, or a "certified approximation" is just an approximation. It does,
    /// one bit per bisection. But it does **not** go to zero: past roughly 54 steps the search is
    /// finer than the `cos`/`sin`/`π` enclosures it is measured against, and `δ` settles on the
    /// accumulated `ROUND_BITS` rounding of those. The floor is asserted as a *fact about the
    /// enclosure*, not hidden behind a loose bound — a test that only checked "smaller than last
    /// time" would keep passing if the floor silently rose by four orders of magnitude.
    #[test]
    fn refinement_shrinks_the_certified_delta_to_the_enclosure_floor() {
        let delta_at = |iters: usize| {
            from_centre_angles::<Bignum>(
                [Q::from_i128(0), Q::from_i128(0)],
                &Q::from_i128(1),
                &Q::from_i128(37),
                &Q::from_i128(64),
                &ArcTolerance::report_only().with_refinement(iters),
            )
            .expect("certified")
            .delta
        };

        // One bit per step, all the way down to the floor.
        let mut previous: Option<Q> = None;
        for iters in [4usize, 8, 16, 32, 64] {
            let d = delta_at(iters);
            if let Some(p) = &previous {
                assert!(
                    d < *p,
                    "delta did not improve from {} to {} at {iters} iters",
                    f(p),
                    f(&d)
                );
            }
            previous = Some(d);
        }
        let floor = previous.expect("ran");
        assert!(
            floor < q(1, 1_000_000_000_000_000i128),
            "64 bisections should be under a femtometre, got {}",
            f(&floor)
        );

        // …and it really is a floor: half again as much work buys essentially nothing, because the
        // limit is the enclosure and not the search.
        let deeper = delta_at(96);
        assert!(
            deeper > floor.mul(&q(1, 2)),
            "delta {} at 96 iters is far below the {} floor — the floor moved, so the enclosure \
             changed and this bound is no longer describing the same thing",
            f(&deeper),
            f(&floor)
        );
    }

    /// Degeneracies refuse by name rather than producing a shape.
    #[test]
    fn degenerate_input_refuses_by_name() {
        let p = [Q::from_i128(1), Q::from_i128(1)];
        assert!(matches!(
            from_bulge::<Bignum>(p.clone(), p.clone(), &Q::from_i128(1)),
            Err(ArcFault::DegenerateChord)
        ));
        assert!(matches!(
            from_bulge::<Bignum>(
                [Q::from_i128(0), Q::from_i128(0)],
                p.clone(),
                &Q::from_i128(0)
            ),
            Err(ArcFault::StraightBulge)
        ));
        assert!(matches!(
            from_centre_angles::<Bignum>(
                [Q::from_i128(0), Q::from_i128(0)],
                &Q::from_i128(0),
                &Q::from_i128(0),
                &Q::from_i128(90),
                &ArcTolerance::report_only()
            ),
            Err(ArcFault::NonPositiveRadius)
        ));
        assert!(matches!(
            from_endpoints_radius::<Bignum>(
                p.clone(),
                p,
                &Q::from_i128(1),
                false,
                true,
                &ArcTolerance::report_only()
            ),
            Err(ArcFault::DegenerateChord)
        ));
    }
}
