//! The **self-lapping cone** — the flex-PCB spine's acceptance demo: ONE connected development chart
//! that, cut from a single sheet and folded, becomes a cone whose offset tail laps over its head.
//!
//! Run: `cargo run --example self_lapping_cone` (writes `generated-demos/self_lapping_cone.svg`).
//! Test: `cargo test --example self_lapping_cone`.
//!
//! **Why one chart, no atlas.** The device cone `q=(9,4,4σ,9σ)` sweeps its Gauss circle *once* over
//! `σ∈ℝ` with the seam stranded at `σ=±∞` (where `|n′|→0` stalls); to lap you need *more* than one
//! turn, which a single degree-1 chart cannot reach. [`cone_wrap`] is the **same cone** parametrized
//! by a degree-2 quaternion that stays in the cone's 2-plane, so it traverses the circle **twice**
//! (`φ=4·arctan σ`) — one turn-plus-lap now fits the finite window `σ∈[−5/4, 5/4]`, the seam at the
//! regular `σ=±1`. Its angle law stays closed-form `ψ=(260/97)·arctan σ`.
//!
//! **Piecewise support, one flat frame.** The body is a true cone (`h≡0`); a §8 smoothstep ramps
//! `h:0→D` near the seam (`h′=0` at both ends, so the surface is C¹ and the development gap-free); the
//! tail plateaus at `h≡D` (the lap offset). `ρ` and `ψ` are support-independent (shared), and `γ` is
//! built as one **cumulative grid** over the σ-samples — each region integrating its own `γ′` on top
//! of the running frame — so the rails are one connected outline by construction. Each flat point is
//! `D=γ(σ)+µ̂ρe(ψ)` via [`ConeDevelopment::point_from`].
//!
//! **Boundaries by intersection.** Outer/inner edges are the cone cut by the parallels `{z=z_out}`,
//! `{z=z_in}`; each region's rail is the exact `µ̂=(z−c_z)/r_z` solve on that region's (offset)
//! surface — continuous across the joins where the pedal `c` matches.

use develop::cone::{ConeDevelopment, DevConfig, FlatBox};
use develop::interval::RatIv;
use fixtures::devices::cone_wrap;
use geom::chart::Chart;
use lattice::{Bignum, Poly, Rat, RatFunc};

type Q = Rat<Bignum>;

/// Rational parameters of the self-lapping demo. `σ_a`, `σ_b` must land on the sampling grid so each
/// γ-increment lies in one region — [`WrapDemo::demo`] and the grid counts are chosen to guarantee it.
#[derive(Clone)]
struct WrapDemo {
    sigma_min: Q,
    sigma_a: Q,
    sigma_b: Q,
    sigma_max: Q,
    /// The `{z=const}` plane-cut fallback boundaries — used only when `diagnostics` is off (the real
    /// demo cuts real cylinders); read only in the non-diagnostics `trim_rails`.
    #[cfg_attr(feature = "diagnostics", allow(dead_code))]
    z_out: Q,
    #[cfg_attr(feature = "diagnostics", allow(dead_code))]
    z_in: Q,
    d: Q,
    /// Quadrature subintervals per γ-grid increment (a short σ-step ⇒ tight even at modest counts).
    panels: usize,
}

impl WrapDemo {
    /// The tuned device: `β≈42°`, a `≈275°` flat sector with a `≈35°` lap, `D=1/10` thin offset.
    /// `σ_a=1/2` and `σ_b=1` sit at grid fractions `7/10`, `9/10` of `[−5/4, 5/4]`.
    fn demo() -> Self {
        WrapDemo {
            sigma_min: Q::new(-5, 4),
            sigma_a: Q::new(1, 2),
            sigma_b: Q::from_i128(1),
            sigma_max: Q::new(5, 4),
            z_out: Q::new(-17, 5),
            z_in: Q::new(-12, 5),
            d: Q::new(1, 10),
            panels: 20,
        }
    }
}

/// The cubic smoothstep support `h(σ)=D·(3t²−2t³)`, `t=(σ−σ_a)/(σ_b−σ_a)`: `h(σ_a)=0`, `h(σ_b)=D`,
/// `h′=0` at *both* ends — the C¹ §8 ramp joining the `h≡0` body and `h≡D` plateau with matching pedal
/// (gap-free development). Cubic (not quintic smootherstep) keeps the global-σ polynomial well-
/// conditioned, so the interval γ-quadrature stays tight; the price is a curvature (`h″`) jump at the
/// joins — a mild crease, not a gap.
fn ramp_support(demo: &WrapDemo) -> RatFunc<Bignum> {
    let inv = Q::from_i128(1).div(&demo.sigma_b.sub(&demo.sigma_a));
    let tp = Poly::from_coeffs(vec![demo.sigma_a.neg(), Q::from_i128(1)]).scale(&inv);
    let t2 = tp.mul(&tp);
    let t3 = t2.mul(&tp);
    let s = t2.scale(&Q::from_i128(3)).sub(&t3.scale(&Q::from_i128(2)));
    RatFunc::from_poly(s.scale(&demo.d))
}

/// The wrapping frame [`cone_wrap`] carrying support `h`.
fn wrap_chart(h: RatFunc<Bignum>) -> Chart<Bignum> {
    Chart::new(cone_wrap().quaternion().clone(), h)
}

/// The exact plane-cut rail `µ̂(σ)=(z₀−c_z)/r_z` — the intersection of the (offset) surface with the
/// parallel `{z=z₀}`, solved exactly over ℚ. This is the `diagnostics`-off fallback trim only; it is a
/// circle on a *cone* but spirals on the offset tail (the very shortcut the cylinder cut replaces).
#[cfg_attr(feature = "diagnostics", allow(dead_code))]
fn plane_rail(chart: &Chart<Bignum>, z0: &Q) -> RatFunc<Bignum> {
    let cz = chart.pedal().comp(2);
    let rz = chart.ruling().comp(2);
    RatFunc::from_poly(Poly::constant(z0.clone()))
        .sub(&cz)
        .div(&rz)
}

/// The trim cylinders. **D1** outer is *concentric* (`cx=cy=0`, radius `√R²`): a **real vertical
/// cylinder** cut is a circle in xy on *any* surface, so the offset tail's outer edge stays a true
/// circle. (`concentric_disk`'s `{z=d}` plane cut is only a circle *on a cone* — a cone-specific
/// optimization that spirals the outer edge outward through the offset ramp; that shortcut is why the
/// plane-cut outer was wrong.) **D2** inner is *eccentric* (contains the apex, off-centre), so
/// `D1 − D2` is an **offset** annulus. Spike-tuned for a real cut across the whole σ-window and a valid
/// (inner-inside-outer) annulus.
#[cfg(feature = "diagnostics")]
fn d1_outer() -> (Q, Q, Q) {
    (Q::from_i128(0), Q::from_i128(0), Q::new(471, 50)) // concentric cylinder, R ≈ 3.069
}
#[cfg(feature = "diagnostics")]
fn d2_inner() -> (Q, Q, Q) {
    (Q::from_i128(0), Q::new(1, 2), Q::from_i128(4)) // eccentric cylinder, contains the apex
}

#[cfg(feature = "diagnostics")]
fn region_bands(demo: &WrapDemo) -> [(Q, Q); 3] {
    [
        (demo.sigma_min.clone(), demo.sigma_a.clone()),
        (demo.sigma_a.clone(), demo.sigma_b.clone()),
        (demo.sigma_b.clone(), demo.sigma_max.clone()),
    ]
}

/// A per-region **certified cylinder-cut rail** `µ̂(σ)` for the vertical cylinder
/// `(x−cx)²+(y−cy)²=r²` — the cone∩cylinder cut is a *surd* in µ̂, so `certified_rail` fits and proves
/// a polynomial rail. The device band is `µ̂<0`, so the boundary is the **Lower** (negative) root (the
/// Upper root sits on the far side of the apex). A lower fit degree keeps the Vandermonde well-
/// conditioned on the narrow, off-centre ramp/tail bands. Needs `--features diagnostics`.
#[cfg(feature = "diagnostics")]
fn cyl_rails(
    charts: [&Chart<Bignum>; 3],
    bands: &[(Q, Q); 3],
    (cx, cy, r2): (Q, Q, Q),
    cfg: &DevConfig<Bignum>,
    label: &str,
) -> [RatFunc<Bignum>; 3] {
    use certify_core::Verdict;
    use export::cut_oracle::RootPick;
    use export::trim::{RailFit, certified_rail, eccentric_disk};
    use lattice::Interval;

    let clearance = Q::from_i128(1);
    let fit = RailFit {
        degree: 4,
        subdiv: 160,
        bits: 44,
    };
    let mk = |i: usize| -> RatFunc<Bignum> {
        let disk = eccentric_disk(cx.clone(), cy.clone(), r2.clone(), RootPick::Lower);
        let span = Interval {
            lo: bands[i].0.clone(),
            hi: bands[i].1.clone(),
        };
        match certified_rail(charts[i], &disk, &span, fit, &clearance, cfg) {
            Verdict::Verified((mu, eps)) => {
                eprintln!("    [{label}, region {i}] Verified  ε≈{:.2e}", qf(&eps));
                mu
            }
            other => panic!("{label} rail (region {i}) did not certify: {other:?}"),
        }
    };
    [mk(0), mk(1), mk(2)]
}

/// Outer (**D1**, concentric circle) and inner (**D2**, eccentric) boundary rails per region.
#[cfg(feature = "diagnostics")]
fn trim_rails(
    charts: [&Chart<Bignum>; 3],
    demo: &WrapDemo,
    cfg: &DevConfig<Bignum>,
) -> ([RatFunc<Bignum>; 3], [RatFunc<Bignum>; 3]) {
    let bands = region_bands(demo);
    let outer = cyl_rails(charts, &bands, d1_outer(), cfg, "D1 concentric outer");
    let inner = cyl_rails(charts, &bands, d2_inner(), cfg, "D2 eccentric inner");
    (outer, inner)
}

/// Concentric `{z=const}` plane-cut fallback (both boundaries) — used only when `diagnostics` is off,
/// so the default build still compiles; the real demo (offset annulus) needs `--features diagnostics`.
#[cfg(not(feature = "diagnostics"))]
fn trim_rails(
    charts: [&Chart<Bignum>; 3],
    demo: &WrapDemo,
    _cfg: &DevConfig<Bignum>,
) -> ([RatFunc<Bignum>; 3], [RatFunc<Bignum>; 3]) {
    let outer = [
        plane_rail(charts[0], &demo.z_out),
        plane_rail(charts[1], &demo.z_out),
        plane_rail(charts[2], &demo.z_out),
    ];
    let inner = [
        plane_rail(charts[0], &demo.z_in),
        plane_rail(charts[1], &demo.z_in),
        plane_rail(charts[2], &demo.z_in),
    ];
    (outer, inner)
}

fn zero2() -> [RatIv<Bignum>; 2] {
    let z = RatIv::point(Q::from_i128(0));
    [z.clone(), z]
}

fn add2(a: &[RatIv<Bignum>; 2], b: &[RatIv<Bignum>; 2]) -> [RatIv<Bignum>; 2] {
    [a[0].add(&b[0]).rounded(), a[1].add(&b[1]).rounded()]
}

/// The three region developments of the wrapping cone, glued into one flat frame by the γ-grid.
struct SelfLapping {
    demo: WrapDemo,
    cfg: DevConfig<Bignum>,
    body: ConeDevelopment<Bignum>,
    ramp: ConeDevelopment<Bignum>,
    tail: ConeDevelopment<Bignum>,
    charts: [Chart<Bignum>; 3],
    out_rails: [RatFunc<Bignum>; 3],
    in_rails: [RatFunc<Bignum>; 3],
}

impl SelfLapping {
    fn new(demo: &WrapDemo) -> Self {
        // A fab-plausible budget: sub-micron enclosures suffice; lighter series/√ keep it fast.
        let cfg = DevConfig {
            terms: 14,
            sqrt_eps: Q::new(1, 1_000_000_000),
        };
        let body_chart = wrap_chart(RatFunc::zero());
        let ramp_chart = wrap_chart(ramp_support(demo));
        let tail_chart = wrap_chart(RatFunc::from_poly(Poly::constant(demo.d.clone())));

        let body =
            ConeDevelopment::new(&body_chart).expect("wrapping cone is a canonical arctan cone");
        let ramp = ConeDevelopment::new_developable(&ramp_chart, demo.panels)
            .expect("ramp shares the cone Gauss circle");
        let tail = ConeDevelopment::new_developable(&tail_chart, demo.panels)
            .expect("plateau shares the cone Gauss circle");

        let (out_rails, in_rails) = trim_rails([&body_chart, &ramp_chart, &tail_chart], demo, &cfg);

        SelfLapping {
            demo: demo.clone(),
            cfg,
            body,
            ramp,
            tail,
            charts: [body_chart, ramp_chart, tail_chart],
            out_rails,
            in_rails,
        }
    }

    /// The 3-D boundary point `X(σ, µ̂) = c(σ) + µ̂·r(σ)` on the region's (offset) surface — the point
    /// the flat sample develops *from*. This is the folded shape: the self-lapping cone itself.
    fn surface3d(&self, sigma: &Q, mu: &Q) -> [f64; 3] {
        let x = self.charts[self.region(sigma)]
            .surface(mu, &Q::from_i128(0))
            .eval(sigma)
            .expect("surface has no pole on the window");
        [qf(&x[0]), qf(&x[1]), qf(&x[2])]
    }

    /// `0` body, `1` ramp, `2` tail (joins go to the earlier region).
    fn region(&self, sigma: &Q) -> usize {
        if *sigma <= self.demo.sigma_a {
            0
        } else if *sigma <= self.demo.sigma_b {
            1
        } else {
            2
        }
    }

    /// The uniform σ-samples `σ_min → σ_max` (`segments+1` of them). `segments` must be a multiple of
    /// 10 so `σ_a` (`7/10`) and `σ_b` (`9/10`) land on the grid.
    fn sigmas(&self, segments: usize) -> Vec<Q> {
        (0..=segments)
            .map(|i| {
                let t = Q::new(i as i128, segments as i128);
                self.demo
                    .sigma_min
                    .add(&self.demo.sigma_max.sub(&self.demo.sigma_min).mul(&t))
            })
            .collect()
    }

    /// The **cumulative** flat directrix `γ(σ_i)` at each sample: `0` on the body, then each ramp/tail
    /// step adds `∫_{σ_{i-1}}^{σ_i} γ′` from that region's development. Short steps keep every
    /// increment's enclosure tight; the running sum is one continuous frame (the connectedness).
    fn gamma_grid(&self, sigmas: &[Q]) -> Vec<[RatIv<Bignum>; 2]> {
        let mut grid = Vec::with_capacity(sigmas.len());
        let mut acc = zero2();
        grid.push(acc.clone());
        for i in 1..sigmas.len() {
            let (prev, s) = (&sigmas[i - 1], &sigmas[i]);
            let inc = match self.region(s) {
                0 => zero2(), // body: γ ≡ 0
                1 => self
                    .ramp
                    .directrix_between(prev, s, &self.cfg)
                    .expect("ramp γ pole"),
                _ => self
                    .tail
                    .directrix_between(prev, s, &self.cfg)
                    .expect("tail γ pole"),
            };
            acc = add2(&acc, &inc);
            grid.push(acc.clone());
        }
        grid
    }

    /// The certified flat point `D=γ+µ̂ρe(ψ)`. `ρ`,`ψ` are support-independent (the shared `body`
    /// development); `γ` comes from the grid. `point_from` with `lo=hi=σ` adds no integration.
    fn point_at(&self, gamma: &[RatIv<Bignum>; 2], sigma: &Q, mu: &Q) -> FlatBox<Bignum> {
        self.body
            .point_from(gamma, sigma, sigma, mu, &self.cfg)
            .expect("shared cone development has no pole")
    }

    fn rail_mu(&self, sigma: &Q, outer: bool) -> Q {
        let rails = if outer {
            &self.out_rails
        } else {
            &self.in_rails
        };
        rails[self.region(sigma)]
            .eval(sigma)
            .expect("boundary rail has no pole on the window")
    }

    /// The one connected boundary of the annular sector: outer rail `σ_min→σ_max`, then inner rail
    /// `σ_max→σ_min` (the two σ-caps are the straight edges the loop closes with). Returns the box
    /// centers, the max backward error, and the developed samples (for the fold round-trip).
    fn outline(&self, segments: usize) -> Outline {
        let sigmas = self.sigmas(segments);
        let gamma = self.gamma_grid(&sigmas);
        let mut eps = Q::from_i128(0);
        let mut ring = Vec::with_capacity(2 * sigmas.len());
        let mut samples = Vec::with_capacity(2 * sigmas.len());
        // outer: σ_min → σ_max; inner: σ_max → σ_min.
        let order = (0..sigmas.len())
            .map(|i| (i, true))
            .chain((0..sigmas.len()).rev().map(|i| (i, false)));
        for (i, outer) in order {
            let mu = self.rail_mu(&sigmas[i], outer);
            let fb = self.point_at(&gamma[i], &sigmas[i], &mu);
            let e = fb.backward_error();
            if e > eps {
                eps = e;
            }
            let (x, y) = fb.center();
            ring.push([x, y]);
            samples.push((sigmas[i].clone(), mu, fb));
        }
        Outline {
            ring,
            eps,
            samples,
            n: segments + 1,
        }
    }

    /// The largest mismatch between a flat boundary chord and the corresponding **3-D** chord over all
    /// consecutive same-rail samples — the isometry corroboration (an oracle ∧ audit, float only). The
    /// certified development is a local isometry, so flat chord ≈ 3-D chord; a systematic gap would
    /// mean the flat pattern does *not* fold back to this surface. Tiny ⇒ the connected sheet folds to
    /// the self-lapping cone.
    fn isometry_defect(&self, o: &Outline) -> f64 {
        let mut worst = 0.0f64;
        // samples are [outer: 0..n] then [inner: n..2n]; compare within each rail only (not the caps).
        for &(a, b) in &[(0usize, o.n), (o.n, 2 * o.n)] {
            for i in a..b - 1 {
                let (s0, m0, fb0) = &o.samples[i];
                let (s1, m1, fb1) = &o.samples[i + 1];
                let (c0, c1) = (fb0.center(), fb1.center());
                let flat =
                    ((qf(&c0.0) - qf(&c1.0)).powi(2) + (qf(&c0.1) - qf(&c1.1)).powi(2)).sqrt();
                let (x0, x1) = (self.surface3d(s0, m0), self.surface3d(s1, m1));
                let d3 =
                    ((x0[0] - x1[0]).powi(2) + (x0[1] - x1[1]).powi(2) + (x0[2] - x1[2]).powi(2))
                        .sqrt();
                worst = worst.max((flat - d3).abs());
            }
        }
        worst
    }

    /// The folded 3-D boundary points (the self-lapping cone), one per outline sample.
    fn folded3d(&self, o: &Outline) -> Vec<[f64; 3]> {
        o.samples
            .iter()
            .map(|(s, m, _)| self.surface3d(s, m))
            .collect()
    }

    /// The flat sector angle (degrees): `c·(arctan σ_max − arctan σ_min)`.
    fn sector_deg(&self) -> f64 {
        let c = 260.0 / 97.0;
        c * (qf(&self.demo.sigma_max).atan() - qf(&self.demo.sigma_min).atan()) * 180.0
            / std::f64::consts::PI
    }

    // ---- interior holes (needs `--features diagnostics`, the export::trim cut bridge) -----------

    /// The region-aware flat directrix `γ(σ)` at an *arbitrary* σ (off the boundary grid): `0` on the
    /// body, the ramp/tail accumulation elsewhere — for developing interior holes.
    #[cfg(feature = "diagnostics")]
    fn gamma_at(&self, sigma: &Q) -> [RatIv<Bignum>; 2] {
        let pole = "hole γ has no pole on the window";
        match self.region(sigma) {
            0 => zero2(),
            1 => self
                .ramp
                .directrix_between(&self.demo.sigma_a, sigma, &self.cfg)
                .expect(pole),
            _ => {
                let base = self
                    .ramp
                    .directrix_between(&self.demo.sigma_a, &self.demo.sigma_b, &self.cfg)
                    .expect(pole);
                let inc = self
                    .tail
                    .directrix_between(&self.demo.sigma_b, sigma, &self.cfg)
                    .expect(pole);
                add2(&base, &inc)
            }
        }
    }

    /// Develop one `(σ, µ̂)` to a flat center in the connected frame, at an arbitrary σ.
    #[cfg(feature = "diagnostics")]
    fn point_any(&self, sigma: &Q, mu: &Q) -> [Q; 2] {
        let fb = self
            .body
            .point_from(&self.gamma_at(sigma), sigma, sigma, mu, &self.cfg)
            .expect("hole development has no pole");
        let (x, y) = fb.center();
        [x, y]
    }

    /// The **3-D seam drill** — a vertical cylinder over the lap overlap that pierces the sheet
    /// **twice**: once through the head (region 0) and once through the offset tail flap above it
    /// (region 2). The two flat holes land far apart in the pattern but coincide in 3-D — cut them,
    /// fold, and the drill lines up through both sheets.
    #[cfg(feature = "diagnostics")]
    fn seam_drill(&self) -> Vec<DrillHole> {
        let (cx, cy, r2) = Self::drill_params();
        [0usize, 2]
            .iter()
            .filter_map(|&ri| {
                let h = self.drill_hole(ri, &cx, &cy, &r2, 24);
                eprintln!(
                    "    [seam drill: region {ri}] {}",
                    if h.is_some() {
                        "cut"
                    } else {
                        "missed (no 2-root σ-extent)"
                    }
                );
                h
            })
            .collect()
    }

    /// The drill center + squared radius on the lap overlap `(−0.5, 2.7)` — where the head (σ≈−0.9) and
    /// the offset tail plateau (σ≈1.1) share the xy (the face-to-face lap); σ≈1.1 keeps the tail hole
    /// clear of the ramp/tail join so its σ-extent is a clean 2-root interval inside the plateau.
    #[cfg(feature = "diagnostics")]
    fn drill_params() -> (Q, Q, Q) {
        (Q::new(-1, 2), Q::new(27, 10), Q::new(1, 40))
    }

    /// The interior hole of a **vertical cylinder** `(x−cx)²+(y−cy)²=r²` cut through region `ri`'s
    /// (offset) surface — the exact `surface∩cylinder`, a quadratic `A µ̂²+B µ̂+C=0` in the *real*
    /// surface point `c+µ̂r` (near/far branches `µ̂=(−B∓√disc)/2A`), its σ-extent the two roots of
    /// `disc(σ)=0`. This reads the true surface, so it is correct on the offset tail and under the
    /// wrapping — unlike `export::trim::hole_loop`, whose apex-ray `tangent_poly` assumes a cone
    /// through the origin (a shortcut that mis-cuts the tail and double-counts the wrap). `None` if the
    /// cylinder does not cut a clean 2-root interval in the region. Point-sampled (the branch is a
    /// surd); the flat centers develop through the connected frame.
    #[cfg(feature = "diagnostics")]
    fn drill_hole(&self, ri: usize, cx: &Q, cy: &Q, r2: &Q, segments: usize) -> Option<DrillHole> {
        let chart = &self.charts[ri];
        let konst = |q: &Q| RatFunc::from_poly(Poly::constant(q.clone()));
        let (rx, ry) = (chart.ruling().comp(0), chart.ruling().comp(1));
        let dx = chart.pedal().comp(0).sub(&konst(cx));
        let dy = chart.pedal().comp(1).sub(&konst(cy));
        let a = rx.mul(&rx).add(&ry.mul(&ry));
        let b = dx.mul(&rx).add(&dy.mul(&ry)).scale(&Q::from_i128(2));
        let c = dx.mul(&dx).add(&dy.mul(&dy)).sub(&konst(r2));
        let disc = b.mul(&b).sub(&a.mul(&c).scale(&Q::from_i128(4))); // disc > 0 inside the hole
        let band = &region_bands(&self.demo)[ri];
        let (s1, s2) = two_roots(&disc, &band.0, &band.1)?;
        // near/far µ̂ = (−B ∓ √disc)/(2A), evaluated in f64 (the branch is a surd), snapped to ℚ.
        let branch = |s: &Q, sign: f64| -> Q {
            let av = qf(&a.eval(s).unwrap_or_else(|| Q::from_i128(1)));
            let bv = qf(&b.eval(s).unwrap_or_else(|| Q::from_i128(0)));
            let dv = qf(&disc.eval(s).unwrap_or_else(|| Q::from_i128(0))).max(0.0);
            q_from_f64((-bv + sign * dv.sqrt()) / (2.0 * av))
        };
        let mut smu = Vec::with_capacity(2 * segments + 2);
        let mut flat = Vec::with_capacity(2 * segments + 2);
        let mut add = |this: &Self, s: Q, sign: f64| {
            let mu = branch(&s, sign);
            flat.push(this.point_any(&s, &mu));
            smu.push((s, mu));
        };
        // One clean CW loop: near branch `s1→s2`, then the *interior* far points `s2→s1` (the shared
        // tips `near(s1)=far(s1)` and `near(s2)=far(s2)` — where disc=0 — are included once each via the
        // near branch, so no degenerate edge). Reversed so the hole winds CW in (σ,µ̂).
        for i in 0..=segments {
            let t = Q::new(i as i128, segments as i128);
            add(self, s1.add(&s2.sub(&s1).mul(&t)), -1.0);
        }
        for i in 1..segments {
            let t = Q::new(i as i128, segments as i128);
            add(self, s2.add(&s1.sub(&s2).mul(&t)), 1.0);
        }
        smu.reverse();
        flat.reverse();
        Some(DrillHole { smu, flat })
    }

    /// A **2-D polygonal hole** authored directly in the flat pattern — a hexagon centred on the flat
    /// image of a mid-band `(σ,µ̂)` point (so it lands inside the annulus), with straight edges in the
    /// 2-D layout (an ECAD cutout).
    #[cfg(feature = "diagnostics")]
    fn flat_polygon(&self) -> Vec<[f64; 2]> {
        let c = self.point_any(&Q::from_i128(0), &Q::new(-23, 20)); // σ=0, µ̂=−1.15: mid-band
        let (cx, cy) = (qf(&c[0]), qf(&c[1]));
        let rr = 0.42;
        (0..6)
            .map(|k| {
                let a = std::f64::consts::PI * (k as f64) / 3.0;
                [cx + rr * a.cos(), cy + rr * a.sin()]
            })
            .collect()
    }
}

/// A drilled interior hole: the region it pierces, its σ-extent `[s1, s2]`, the boundary `(σ,µ̂)`
/// samples (near branch `s1→s2` then far branch `s2→s1`), and the developed flat ring.
#[cfg(feature = "diagnostics")]
struct DrillHole {
    /// The hole boundary as a `(σ,µ̂)` polygon loop (a general cut, not a near/far band).
    smu: Vec<(Q, Q)>,
    /// The developed flat ring.
    flat: Vec<[Q; 2]>,
}

#[cfg(feature = "diagnostics")]
impl SelfLapping {
    /// The hole boundary folded back to 3-D (the drill through the sheet).
    fn hole_3d(&self, h: &DrillHole) -> Vec<[f64; 3]> {
        h.smu.iter().map(|(s, m)| self.surface3d(s, m)).collect()
    }

    /// The refold residual: the max `|(x−cx)²+(y−cy)²−r²|` over the folded hole — how far the folded
    /// boundary strays from the true drill cylinder. Near 0 ⇒ the flat hole folds back onto the drill
    /// (so the head and tail holes, both on this cylinder, line up through the sheet).
    fn hole_cylinder_defect(&self, h: &DrillHole, cx: &Q, cy: &Q, r2: &Q) -> f64 {
        let (cxf, cyf, r2f) = (qf(cx), qf(cy), qf(r2));
        self.hole_3d(h)
            .iter()
            .map(|p| ((p[0] - cxf).powi(2) + (p[1] - cyf).powi(2) - r2f).abs())
            .fold(0.0, f64::max)
    }

    /// Fold a **flat** point back to `(σ,µ̂)` on the body (`γ≡0`) — the analytic inverse of the signed
    /// development `D=µ̂·ρ·e(ψ)` with `µ̂<0` (so the flat point sits at angle `ψ+π`): `ψ=atan2(y,x)−π`,
    /// `σ=tan(ψ/c)`, `µ̂=−|D|/ρ(σ)`. Lets a 2-D-authored polygon be cut into the STEP surface.
    #[cfg_attr(not(feature = "step"), allow(dead_code))]
    fn fold_flat(&self, p: [f64; 2]) -> (Q, Q) {
        use std::f64::consts::PI;
        let c = 260.0 / 97.0;
        let mut psi = p[1].atan2(p[0]) - PI; // µ̂<0 ⇒ flat angle = ψ+π
        if psi < -PI {
            psi += 2.0 * PI; // unwrap onto the principal branch (the hex straddles the −x axis)
        }
        let sigma = q_from_f64((psi / c).tan());
        let rho = qf(&self.body.radius(&sigma, &Q::new(1, 1_000_000_000)).mid());
        let mu = q_from_f64(-(p[0] * p[0] + p[1] * p[1]).sqrt() / rho);
        (sigma, mu)
    }

    /// The 2-D hexagon folded to a `(σ,µ̂)` polygon (for cutting into the STEP surface).
    #[cfg_attr(not(feature = "step"), allow(dead_code))]
    fn hex_poly(&self) -> Vec<(Q, Q)> {
        self.flat_polygon()
            .iter()
            .map(|&p| self.fold_flat(p))
            .collect()
    }
}

/// A developed boundary: the polygon centers, the certified backward error, the raw samples, and the
/// index count `n` per rail (outer = `0..n`, inner = `n..2n`).
struct Outline {
    ring: Vec<[Q; 2]>,
    eps: Q,
    samples: Vec<(Q, Q, FlatBox<Bignum>)>,
    n: usize,
}

/// A rational value as `f64` (diagnostic / SVG only — never in a certificate).
fn qf(r: &Q) -> f64 {
    let (n, d) = r.numer_denom_decimal();
    n.parse::<f64>().unwrap() / d.parse::<f64>().unwrap()
}

/// A rational approximation of a float (6 decimal digits) — for authoring hole geometry off the surd
/// branch. Diagnostics-only (floats are permitted behind that flag).
#[cfg(feature = "diagnostics")]
fn q_from_f64(x: f64) -> Q {
    Q::new((x * 1_000_000.0).round() as i128, 1_000_000)
}

/// The first two sign-change roots of a RatFunc on `[lo, hi]` — a coarse f64 scan + bisection (used to
/// bound a drill hole's σ-extent, where `disc(σ)` crosses zero). `None` if fewer than two are found.
#[cfg(feature = "diagnostics")]
fn two_roots(f: &RatFunc<Bignum>, lo: &Q, hi: &Q) -> Option<(Q, Q)> {
    let (lof, hif) = (qf(lo), qf(hi));
    let ev = |x: f64| qf(&f.eval(&q_from_f64(x)).unwrap_or_else(|| Q::from_i128(0)));
    let mut roots = Vec::new();
    let (mut px, mut ps) = (lof, ev(lof).signum());
    for k in 1..=400usize {
        let x = lof + (hif - lof) * (k as f64) / 400.0;
        let s = ev(x).signum();
        if ps != 0.0 && s != 0.0 && ps != s {
            let (mut a, mut b) = (px, x);
            for _ in 0..40 {
                let m = 0.5 * (a + b);
                if ev(m).signum() == ps {
                    a = m;
                } else {
                    b = m;
                }
            }
            roots.push(q_from_f64(0.5 * (a + b)));
        }
        px = x;
        ps = s;
    }
    if roots.len() >= 2 {
        Some((roots[0].clone(), roots[1].clone()))
    } else {
        None
    }
}

fn ring_f64(ring: &[[Q; 2]]) -> Vec<[f64; 2]> {
    ring.iter().map(|p| [qf(&p[0]), qf(&p[1])]).collect()
}

/// A minimal self-contained SVG of the flat pattern: each ring is one `evenodd` sub-path (interior
/// rings cut holes), fitted to the point bounds with a math-up y-flip.
fn write_svg(rings: &[Vec<[f64; 2]>], px: f64) -> String {
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for r in rings {
        for p in r {
            minx = minx.min(p[0]);
            miny = miny.min(p[1]);
            maxx = maxx.max(p[0]);
            maxy = maxy.max(p[1]);
        }
    }
    let (w, h) = (maxx - minx, maxy - miny);
    let pad = 0.06 * w.max(h);
    let (minx, miny) = (minx - pad, miny - pad);
    let (w, h) = (w + 2.0 * pad, h + 2.0 * pad);
    let hpx = (px * h / w).round().max(1.0);
    let flip = 2.0 * miny + h;
    let sw = 0.004 * w.max(h);
    let mut d = String::new();
    for r in rings {
        for (i, p) in r.iter().enumerate() {
            d.push_str(if i == 0 { "M" } else { "L" });
            d.push_str(&format!("{:.5} {:.5} ", p[0], p[1]));
        }
        d.push_str("Z ");
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{px:.0}\" height=\"{hpx:.0}\" \
         viewBox=\"{minx:.5} {miny:.5} {w:.5} {h:.5}\">\
         <g transform=\"matrix(1 0 0 -1 0 {flip:.5})\">\
         <path d=\"{d}\" fill=\"#5b8def\" fill-opacity=\"0.35\" fill-rule=\"evenodd\" \
         stroke=\"#1f3b8c\" stroke-width=\"{sw:.5}\" stroke-linejoin=\"round\"/></g></svg>"
    )
}

/// A top-down (`x,y`) projection of the folded 3-D boundary — the rolled-up cone seen along its axis.
/// The tail (offset by `D` along the normal) sweeps *outside* the head at the shared azimuth: the lap
/// is the near-closure where the two ends overlap.
fn write_folded_topdown(rings3d: &[Vec<[f64; 3]>], px: f64) -> String {
    let rings: Vec<Vec<[f64; 2]>> = rings3d
        .iter()
        .map(|r| r.iter().map(|p| [p[0], p[1]]).collect())
        .collect();
    write_svg(&rings, px)
}

/// Emit the folded self-lapping cone as **one connected STEP shell** (needs `--features step` under
/// `nix develop`). The three region charts share the ruling/normal frame and meet with a continuous
/// pedal at `σ_a`, `σ_b`, so `brep_trim_solid_regions` sews them into a single watertight solid — the
/// intersection-trimmed annular band (`{z=z_out}`…`{z=z_in}`) with the offset tail. Prints the OCCT
/// audit (the external differential oracle).
#[cfg(feature = "step")]
fn emit_step(dev: &SelfLapping) {
    use export::brep_build::brep_trim_solid_regions;
    use export::step::{audit_brep, write_brep};
    use lattice::Interval;

    let d = &dev.demo;
    let band = |lo: &Q, hi: &Q| Interval {
        lo: lo.clone(),
        hi: hi.clone(),
    };
    let bands = [
        band(&d.sigma_min, &d.sigma_a),
        band(&d.sigma_a, &d.sigma_b),
        band(&d.sigma_b, &d.sigma_max),
    ];
    let charts = [
        (bands[0].clone(), &dev.charts[0]),
        (bands[1].clone(), &dev.charts[1]),
        (bands[2].clone(), &dev.charts[2]),
    ];
    // The builder's "inner" rail is the more-negative µ̂ (larger apex-radius) — that is the {z=z_out}
    // cut; "outer" is {z=z_in}. Each is piecewise over the three region bands.
    let brep_inner: Vec<_> = (0..3)
        .map(|i| (bands[i].clone(), dev.out_rails[i].clone()))
        .collect();
    let brep_outer: Vec<_> = (0..3)
        .map(|i| (bands[i].clone(), dev.in_rails[i].clone()))
        .collect();
    let w = Interval {
        lo: Q::from_i128(0),
        hi: Q::new(1, 20),
    };
    // The interior cutouts as general `(σ,µ̂)` polygon holes (when the trim bridge is on): the two
    // round seam drills and the folded 2-D hexagon — all cut from their region's lid + walls.
    let poly_holes: Vec<Vec<(Q, Q)>> = {
        #[cfg(feature = "diagnostics")]
        {
            let mut v: Vec<Vec<(Q, Q)>> = dev.seam_drill().iter().map(|h| h.smu.clone()).collect();
            v.push(dev.hex_poly());
            v
        }
        #[cfg(not(feature = "diagnostics"))]
        {
            Vec::new()
        }
    };
    match brep_trim_solid_regions(&charts, &w, &brep_inner, &brep_outer, &[], &poly_holes) {
        Some(solid) => {
            let path = "generated-demos/self_lapping_cone.step";
            write_brep(path, &solid);
            match audit_brep(&solid) {
                Ok(a) => println!(
                    "  STEP shell     : faces={} edges={} free={} nonmanifold={} closed={} valid={}\n  wrote {path}",
                    a.faces,
                    a.edges,
                    a.free_edges,
                    a.nonmanifold_edges,
                    a.closed,
                    a.brepcheck_valid
                ),
                Err(e) => println!("  STEP audit error: {e}"),
            }
        }
        None => println!("  STEP: brep_trim_solid_regions returned None (check bands/rails)"),
    }
}

fn main() {
    let demo = WrapDemo::demo();
    let dev = SelfLapping::new(&demo);
    let o = dev.outline(120);

    std::fs::create_dir_all("generated-demos").ok();

    #[cfg(feature = "diagnostics")]
    let drills = dev.seam_drill();

    // Flat pattern: the outer boundary, then (with the trim bridge) the two seam-drill holes and the
    // 2-D polygon — each an even-odd sub-path that cuts a hole. Folded view: the outline plus the two
    // holes projected to xy (both land on the drill cylinder).
    #[cfg_attr(not(feature = "diagnostics"), allow(unused_mut))]
    let mut rings = vec![ring_f64(&o.ring)];
    #[cfg_attr(not(feature = "diagnostics"), allow(unused_mut))]
    let mut folded_rings = vec![dev.folded3d(&o)];
    #[cfg(feature = "diagnostics")]
    {
        for h in &drills {
            rings.push(ring_f64(&h.flat));
            folded_rings.push(dev.hole_3d(h));
        }
        rings.push(dev.flat_polygon());
    }
    let flat = "generated-demos/self_lapping_cone.svg";
    std::fs::write(flat, write_svg(&rings, 900.0)).expect("write flat svg");
    let folded = "generated-demos/self_lapping_cone_folded.svg";
    std::fs::write(folded, write_folded_topdown(&folded_rings, 900.0)).expect("write folded svg");

    println!("self-lapping cone — one connected development chart");
    println!(
        "  flat sector      : {:.1}°  (one turn ≈ 240.9°, the excess is the lap)",
        dev.sector_deg()
    );
    println!("  boundary vertices: {}", o.ring.len());
    println!("  flat ε (max)     : {:.3e}", qf(&o.eps));
    println!(
        "  isometry defect  : {:.3e}  (flat chord vs 3-D chord — folds to the cone)",
        dev.isometry_defect(&o)
    );
    #[cfg(feature = "diagnostics")]
    {
        let (cx, cy, r2) = SelfLapping::drill_params();
        let defect = drills
            .iter()
            .map(|h| dev.hole_cylinder_defect(h, &cx, &cy, &r2))
            .fold(0.0, f64::max);
        println!(
            "  refold defect    : {defect:.3e}  (folded holes land on the drill cylinder — through both sheets)"
        );
    }
    println!("  wrote {flat}");
    println!("  wrote {folded}  (top-down view of the folded cone — the tail laps the head)");

    #[cfg(feature = "step")]
    emit_step(&dev);
    #[cfg(not(feature = "step"))]
    println!("  (STEP shell: rerun with `--features step` under `nix develop`)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_development_is_one_connected_outline() {
        let dev = SelfLapping::new(&WrapDemo::demo());
        let o = dev.outline(40);
        assert!(o.ring.len() > 60, "a resolved connected boundary");
        // The cumulative γ-grid makes the outline connected by construction; the enclosure stays
        // fab-plausible across the whole (body + ramp + tail) sweep.
        assert!(
            o.eps < Q::new(1, 20),
            "flat backward error stays small, got {:?}",
            o.eps
        );
    }

    #[test]
    fn the_flat_sector_exceeds_one_turn_by_the_lap() {
        let dev = SelfLapping::new(&WrapDemo::demo());
        // One full 3D turn develops to 2π·sinβ ≈ 240.9°; the sector must exceed it — that excess IS
        // the lap (the tail overlapping the head when rolled up), yet stay a cuttable single sheet.
        let s = dev.sector_deg();
        assert!(
            s > 240.9 && s < 300.0,
            "sector {s:.1}° must lap (>240.9°) yet be < 360°"
        );
    }

    #[test]
    fn the_flat_pattern_folds_back_to_the_cone() {
        let dev = SelfLapping::new(&WrapDemo::demo());
        let o = dev.outline(40);
        // The certified development is a local isometry: flat chords match 3-D chords across the whole
        // body+ramp+tail sweep. So the one connected sheet folds back to the self-lapping cone.
        let defect = dev.isometry_defect(&o);
        assert!(
            defect < 1e-2,
            "flat pattern must fold isometrically, chord defect {defect:e}"
        );
    }

    // The real (offset-annulus + holes) geometry — the `diagnostics` trim bridge.
    // Run: `cargo test --example self_lapping_cone --features diagnostics`.
    #[cfg(feature = "diagnostics")]
    #[test]
    fn the_seam_drill_pierces_both_sheets_and_refolds() {
        let dev = SelfLapping::new(&WrapDemo::demo());
        let drills = dev.seam_drill();
        // The one vertical drill cuts both the head (region 0) and the offset tail flap (region 2).
        assert_eq!(drills.len(), 2, "the seam drill must pierce both sheets");
        // Each flat hole, folded back, lands on the same drill cylinder — so the two line up in 3-D.
        let (cx, cy, r2) = SelfLapping::drill_params();
        let defect = drills
            .iter()
            .map(|h| dev.hole_cylinder_defect(h, &cx, &cy, &r2))
            .fold(0.0, f64::max);
        assert!(
            defect < 1e-3,
            "folded holes must land on the drill cylinder, defect {defect:e}"
        );
        // The 2-D hexagon folds to a (σ,µ̂) polygon strictly inside the body region (a cuttable hole).
        let hex = dev.hex_poly();
        assert_eq!(hex.len(), 6);
        assert!(
            hex.iter()
                .all(|(s, _)| *s > Q::new(-1, 2) && *s < Q::new(1, 2)),
            "the hex must fold into the body region"
        );
    }
}
