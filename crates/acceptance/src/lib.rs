//! The canonical **acceptance parts** — one definition of each device, shared by the demo
//! drivers, the V&V pins, and the benchmark.
//!
//! These devices were previously hand-rolled once per consumer. That is fine until a consumer
//! *measures* one: a benchmark timing a slightly different part than the ε budget pins, or a
//! golden metric reading geometry the demo does not emit, looks green while guarding nothing.
//! Keeping the recipe in one place is what lets a measurement and the check that guards it be
//! about the same object.
//!
//! Resolution is a *parameter*, not part of the recipe — the demo runs the self-lapping cone at a
//! fidelity that takes minutes, the test suite runs the same device lean. Same geometry, same
//! derived structure, different budget.
//!
//! ```no_run
//! let part = acceptance::self_lapping_cone(16, 8, true);
//! assert!(matches!(part.develop(), certify_core::Verdict::Verified(_)));
//! ```

pub mod measure;

use arrange2d::profile::Profile;
use author::construct;
use author::part::{Cutter, Part, SupportFn};
use develop::cone::DevConfig;
use develop::extrude::{Apex, Frame};
use export::trim::RailFit;
use fixtures::devices::{cone, cone_wrap};
use geom::content::Edge;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// The **self-lapping cone**: the driving-demo geometry.
///
/// The wrapping chart `ψ = (260/97)·arctan σ` sweeps more than one full 3-D turn in a finite
/// window, and three piecewise-support regions ride it — body `[−5/4, 1/2]` at `h ≡ 0`, a
/// smoothstep ramp `[1/2, 1]` climbing `0 → D = 1/10`, and a tail plateau `[1, 5/4]` at `h ≡ D`.
/// The excess sweep *is* the lap: the tail passes over the head. Two solid cutters bound the
/// annulus (concentric outer, apex-containing inner) and, with `with_drill`, one seam-drill
/// cylinder pierces the sheet **twice** — once in the head, once in the lapping tail flap — so a
/// single cutter derives two holes that must fold back onto the same 3-D cylinder.
///
/// `segments` sets the boundary resolution and `support_panels` the γ-quadrature budget; the two
/// together decide both the certified ε and the runtime. The body has `h ≡ 0` and so develops with
/// `γ ≡ 0`, while the ramp and tail carry a nonzero flat directrix — which is why this one device
/// exercises both development tiers.
pub fn self_lapping_cone(segments: usize, support_panels: usize, with_drill: bool) -> Part<Bignum> {
    self_lapping_cone_with(segments, support_panels, with_drill, None)
}

/// The same device carrying **one authored feature** swept from `apex` through `profile` — the
/// stress variant.
///
/// `None` reproduces [`self_lapping_cone`] exactly, op for op, which is what keeps the pinned
/// device pinned: the VV.1 work budgets, VV.2 ε bounds and VV.3 chord goldens are all measured on
/// the featureless recipe, and a baseline that moves whenever a new fixture is added stops being a
/// baseline.
///
/// Everything AUTH.1/AUTH.2 has been exercised on so far lives on [`sketch_panel`] — one region,
/// `SupportFn::inherit` (so `γ ≡ 0`), no wrap. This is the hard chart: the extruded footprint is
/// traced over a **nonzero flat directrix**, across a **multi-region** development, on a surface
/// that passes over itself. See [`lap_slot`] for where the feature is placed and why.
pub fn self_lapping_cone_with(
    segments: usize,
    support_panels: usize,
    with_drill: bool,
    feature: Option<(Apex<Bignum>, Vec<Edge<Bignum>>)>,
) -> Part<Bignum> {
    let d = q(1, 10);
    // A witness on the kept sheet: the σ = 0 ruling's point at z = −3 (mid-annulus). The wrap
    // chart keeps material on both sheets of the double cover — the antipodal ray crosses the
    // disks too — so the recipe must designate the component rather than leave it to a rule.
    let rz0 = cone_wrap()
        .ruling()
        .comp(2)
        .eval(&qi(0))
        .expect("the wrap chart's ruling is regular at σ = 0");
    let mu_w = q(-3, 1).div(&rz0);
    let witness = cone_wrap()
        .surface(&mu_w, &qi(0))
        .eval(&qi(0))
        .expect("the mid-annulus witness point is regular");
    let mut part = construct::from_chart::<Bignum>(&cone_wrap())
        .region_sigma(q(-5, 4), q(1, 2), SupportFn::constant(qi(0)))
        .region_sigma(q(1, 2), qi(1), SupportFn::smoothstep(qi(0), d.clone()))
        .region_sigma(qi(1), q(5, 4), SupportFn::constant(d))
        .keep_near(witness)
        .intersect(Cutter::vertical_cylinder(qi(0), qi(0), q(471, 50)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(4)))
        .clearance(qi(1))
        .thickness(q(1, 20))
        .fit(RailFit {
            degree: 4,
            subdiv: 160,
            bits: 44,
        })
        .segments(segments)
        .support_panels(support_panels)
        .budget(DevConfig {
            terms: 14,
            sqrt_eps: q(1, 1_000_000_000),
        });
    if with_drill {
        part = part.subtract(Cutter::vertical_cylinder(q(-1, 2), q(27, 10), q(1, 40)));
    }
    if let Some((apex, profile)) = feature {
        part = part.subtract(Cutter::extrude(sketch_plane(), apex, profile));
    }
    part
}

/// The centre of the self-lapping device's seam drill, `(x, y, r²)` — the 3-D cylinder both
/// derived holes must fold back onto. Exposed so a round-trip check tests the *same* cylinder the
/// part was cut with instead of restating its numbers.
pub fn seam_drill_axis() -> (Q, Q, Q) {
    (q(-1, 2), q(27, 10), q(1, 40))
}

/// The **lap slot**: the L-shaped feature [`self_lapping_cone_with`] is stressed with — arm `1/4`,
/// thickness `1/8`, corner at `(1/2, 109/40)`, on the rotated axes `u = (3/5, −4/5)`,
/// `v = (4/5, 3/5)`.
///
/// Drawn in the `z = 0` [`sketch_plane`] and swept along `z`, so where it lands is decided entirely
/// by its `(x, y)` extent — and every number above is a placement, not a shape:
///
/// * **In the lap wedge.** The device's azimuth sweeps 410.7°, so the wedge `az ∈ (64.6°, 115.4°)`
///   is covered **twice** — once by the body at `h ≡ 0` (`σ ∈ [−5/4, −0.79]`) and once by the ramp
///   and tail flap passing over it (`σ ∈ [0.79, 5/4]`). This slot spans `az ∈ [72.4°, 79.6°]`, so
///   one cutter pierces **both sheets**: a ruling meets its footprint on the near sheet and again on
///   the far one. That is the wrap chart's own version of the multi-stretch problem, and no gore can
///   pose it.
/// * **On the ramp.** The far sheet lands at `σ ∈ [0.857, 0.914]` — strictly inside the smoothstep
///   band `[1/2, 1]` — so that hole is traced over a **nonzero flat directrix**, while its twin on
///   the body is traced at `γ ≡ 0`. One cutter, both development tiers, and the difference between
///   the two holes is attributable to `γ` and to nothing else.
/// * **Clear of the joins.** The near sheet lands at `σ ∈ [−1.168, −1.094]`, ~0.08 from the body's
///   far end; neither footprint crosses a region boundary, which is refused
///   ([`PartFault::HoleCrossesRegions`](author::part::PartFault::HoleCrossesRegions)) rather than
///   realized.
/// * **Inside the annulus.** Its radii `[2.61, 2.96]` sit between the eccentric inner cut (`≈2.47`
///   there) and the outer cylinder (`≈3.07`), so it is an interior hole and not a rim bite.
///
/// The axes are turned for the same reason [`ell_slot`]'s are: the rulings project to radial rays,
/// so an L whose arms lie along the radius is met once and its footprint is an ordinary band. Here
/// the local radial direction resolves to `(−0.63, +0.78)` in `(u, v)` — opposite signs, so a ray
/// leaves one arm, crosses the notch and re-enters the other. `(3, 4, 5)` keeps every vertex exact.
pub fn lap_slot() -> Vec<Edge<Bignum>> {
    let (cx, cy, a, t) = (q(1, 2), q(109, 40), q(1, 4), q(1, 8));
    let (ux, uy) = (q(3, 5), q(-4, 5));
    let (vx, vy) = (q(4, 5), q(3, 5));
    let p = |su: &Q, sv: &Q| {
        [
            cx.add(&ux.mul(su)).add(&vx.mul(sv)),
            cy.add(&uy.mul(su)).add(&vy.mul(sv)),
        ]
    };
    let z = qi(0);
    Profile::new()
        .polygon(&[
            p(&z, &z),
            p(&a, &z),
            p(&a, &t),
            p(&t, &t),
            p(&t, &a),
            p(&z, &a),
        ])
        .into_edges()
}

/// The **sketch-cutter panel**: the AUTH.1/AUTH.2 device — the Stage-1 gore over the full
/// `σ ∈ [−7/2, 7/2]` (~296°), bounded by the `z ≤ 3` half-space and the eccentric apex cylinder,
/// notched at the rim, and carrying **one authored feature** swept from `apex` through `profile`.
///
/// `None` builds the same panel without that feature — the control every faithfulness measurement
/// is read against, so a difference is attributable to the cut and to nothing else.
///
/// The feature is the only interior hole, which is what makes the work counters legible on this
/// device: `develop::counters::poly_slice_clips()` counts the σ-slices the traced footprint
/// reaches, so above 1 says the hole crossed a σ-station.
///
/// ```no_run
/// let part = acceptance::sketch_panel(Some((
///     develop::extrude::Apex::direction([
///         lattice::Rat::from_i128(0),
///         lattice::Rat::from_i128(0),
///         lattice::Rat::from_i128(1),
///     ])
///     .unwrap(),
///     acceptance::ell_slot(),
/// )));
/// assert!(matches!(part.develop(), certify_core::Verdict::Verified(_)));
/// ```
pub fn sketch_panel(cut: Option<(Apex<Bignum>, Vec<Edge<Bignum>>)>) -> Part<Bignum> {
    let base = sketch_gore();
    let base = match cut {
        Some((apex, profile)) => base.subtract(Cutter::extrude(sketch_plane(), apex, profile)),
        None => base,
    };
    base.clearance(qi(1)).thickness(q(1, 8)).segments(72)
}

/// The same panel with a **metric** cylinder `(cx, cy, r²)` in place of the authored feature.
///
/// This is the differential control: `Cutter::vertical_cylinder` reaches the flat pattern through
/// the quadric branch, which shares no line of code with the tracer, so a disc drilled here and an
/// extruded footprint compared against it are two independent constructions rather than one
/// construction restated.
pub fn sketch_drill(cx: Q, cy: Q, r2: Q) -> Part<Bignum> {
    sketch_gore()
        .subtract(Cutter::vertical_cylinder(cx, cy, r2))
        .clearance(qi(1))
        .thickness(q(1, 8))
        .segments(72)
}

/// The `z = 0` sketch plane in world coordinates, orthonormal so a profile circle is a true circle.
pub fn sketch_plane() -> Frame<Bignum> {
    Frame::new(
        [qi(0), qi(0), qi(0)],
        [qi(1), qi(0), qi(0)],
        [qi(0), qi(1), qi(0)],
    )
    .expect("the axes are independent")
}

/// [`sketch_panel`]'s blank, before the feature and the resolution settings.
fn sketch_gore() -> Part<Bignum> {
    // A witness on the kept sheet: the σ = 0 ruling at µ̂ = 2, mid-annulus.
    let witness = cone()
        .surface(&qi(2), &qi(0))
        .eval(&qi(0))
        .expect("the device cone is regular at σ = 0");
    construct::from_chart::<Bignum>(&cone())
        .region_sigma(q(-7, 2), q(7, 2), SupportFn::inherit())
        .keep_near(witness)
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2)))
        .subtract(Cutter::vertical_cylinder(q(-9, 4), q(9, 4), q(9, 16)))
}

/// The **L-slot**: arm `1/4`, thickness `1/8`, corner at `(−1/10, 11/5)`, laid out on the *rotated*
/// axes `u = (4/5, −3/5)`, `v = (3/5, 4/5)`.
///
/// The rotation is geometry, not decoration. This cone's rulings project to **radial** rays, so an
/// L whose arms lie along the radius is met by every ray exactly once — its footprint is an
/// ordinary band and the notch never appears in `(σ, µ̂)` at all. What AUTH.2 lifts is a restriction
/// on *footprints*, so the fixture has to produce one, and the signature that counts is a ruling
/// meeting the cutter **twice** (`docs/cutter-extrude-design.md` §11.6). Nor is a reflex corner in
/// the flat pattern evidence: a band can be a thoroughly non-convex region. Turning the notch
/// *across* the rulings is what does it, and the `(3,4,5)` triple keeps every vertex rational, so
/// the frame is exact rather than a rounded 45°.
pub fn ell_slot() -> Vec<Edge<Bignum>> {
    let (cx, cy, a, t) = (q(-1, 10), q(11, 5), q(1, 4), q(1, 8));
    let (ux, uy) = (q(4, 5), q(-3, 5));
    let (vx, vy) = (q(3, 5), q(4, 5));
    let p = |su: &Q, sv: &Q| {
        [
            cx.add(&ux.mul(su)).add(&vx.mul(sv)),
            cy.add(&uy.mul(su)).add(&vy.mul(sv)),
        ]
    };
    let z = qi(0);
    Profile::new()
        .polygon(&[
            p(&z, &z),
            p(&a, &z),
            p(&a, &t),
            p(&t, &t),
            p(&t, &a),
            p(&z, &a),
        ])
        .into_edges()
}

/// The **keyhole**: a round head of radius `3/20` about `(0, 11/5)` with a straight stem hanging
/// off the chord at `−18/125`, its sides `±21/500` meeting the circle exactly (`7² + 24² = 25²`),
/// on the rotated axes `u = (15/17, 8/17)`, `v = (−8/17, 15/17)`.
///
/// The L is drawn entirely in straight lines; this profile mixes a **quadric** wall with affine
/// ones, and that is what it is for. A ruling crossing it obliquely enters the head, leaves through
/// the notch beside the stem and re-enters the stem, so its two stretches rejoin over a saddle
/// whose two walls are a circle and a straight edge — the mixed-degree case of the pairwise
/// resultant, which no polygon can reach (§11.2). Both the narrow stem and the rotation are chosen
/// so a ruling actually crosses that notch rather than merely being able to in principle.
pub fn keyhole_slot() -> Vec<Edge<Bignum>> {
    let (cx, cy) = (qi(0), q(11, 5));
    let (ux, uy) = (q(15, 17), q(8, 17));
    let (vx, vy) = (q(-8, 17), q(15, 17));
    let p = |su: &Q, sv: &Q| {
        [
            cx.add(&ux.mul(su)).add(&vx.mul(sv)),
            cy.add(&uy.mul(su)).add(&vy.mul(sv)),
        ]
    };
    let (hw, chord, foot) = (q(21, 500), q(18, 125), q(3, 10));
    let a = p(&hw, &chord.clone().neg());
    let b = p(&hw.clone().neg(), &chord.neg());
    let c = p(&hw.clone().neg(), &foot.clone().neg());
    let d = p(&hw, &foot.neg());
    Profile::new()
        .arc(cx, cy, q(9, 400), a.clone(), b.clone())
        .polyline(&[b, c, d, a])
        .into_edges()
}

/// A **ring** profile at `(0, 11/5)` — the scope refusal, kept beside the shapes that work so the
/// fixture and its refusal cannot drift apart. An annular through-cut would leave a disc of
/// material floating free, which is two parts rather than one hole (§11.8).
pub fn ring_slot() -> Vec<Edge<Bignum>> {
    Profile::new()
        .circle(qi(0), q(11, 5), q(1, 5))
        .circle(qi(0), q(11, 5), q(1, 10))
        .into_edges()
}

/// The **radiused panel outline** — a rounded rectangle about `(cx, cy)`, half-extents `(w, h)`,
/// corner radius `r`. The outline a flex fabricator actually accepts: sharp corners are a tear
/// risk, so a real panel boundary is straights joined by radii.
///
/// It is the mixed-wall contour, and that is the point. Four sides are **planes** — affine
/// µ̂-pullbacks that `plane_cut_rail` gives exactly — and four corners are **cylinders**, genuine
/// quadratics whose two branches meet at tangent rulings. One cutter, both wall kinds, so a part
/// kept inside it exercises the exact-rail path and the traced-loop path together.
///
/// Every vertex is exactly on its circle with no Pythagorean split, because the arc endpoints are
/// the axis-aligned tangent points: `(cx ± (w−r), cy ± h)` and `(cx ± w, cy ± (h−r))`. So the
/// profile is exact over ℚ for any rational `r`, which a chamfer-by-chord would not be.
///
/// **Radius is not a free parameter.** The tracer spends its `segments` budget over the *whole*
/// loop, so a small radius starves its own arcs and forces the entire outline finer: measured on the
/// doctest cone, `r = w/5` needs 384 segments to certify at all, while `r = 2w/5` certifies at 48
/// and converges `5.4e-2 → 4.0e-2 → 1.7e-2` over `48 → 96 → 192`. Too *large* fails the other way —
/// at `r = 3w/5` the corners consume the sides and the footprint stops being one µ̂-interval per
/// ruling (`AmbiguousRegion`, §12.5).
pub fn rounded_outline(cx: Q, cy: Q, w: Q, h: Q, r: Q) -> Vec<Edge<Bignum>> {
    let (wi, hi) = (w.sub(&r), h.sub(&r));
    let r2 = r.mul(&r);
    let p = |dx: Q, dy: Q| [cx.add(&dx), cy.add(&dy)];
    let centre = |sx: i128, sy: i128| (cx.add(&wi.mul(&qi(sx))), cy.add(&hi.mul(&qi(sy))));
    let mut pr = Profile::new();
    // CCW from the bottom-left tangent point: side, corner, side, corner, …
    let corners = [
        (
            (1i128, -1i128),
            (wi.clone(), h.neg()),
            (w.clone(), hi.neg()),
        ),
        ((1, 1), (w.clone(), hi.clone()), (wi.clone(), h.clone())),
        ((-1, 1), (wi.neg(), h.clone()), (w.neg(), hi.clone())),
        ((-1, -1), (w.neg(), hi.neg()), (wi.neg(), h.neg())),
    ];
    let mut from = (wi.neg(), h.neg());
    for ((sx, sy), arc_start, arc_end) in corners {
        pr = pr.polyline(&[
            p(from.0.clone(), from.1.clone()),
            p(arc_start.0.clone(), arc_start.1.clone()),
        ]);
        let (bx, by) = centre(sx, sy);
        pr = pr.arc(
            bx,
            by,
            r2.clone(),
            p(arc_start.0, arc_start.1),
            p(arc_end.0.clone(), arc_end.1.clone()),
        );
        from = arc_end;
    }
    pr.into_edges()
}

/// The three **metric probes** that bracket [`ell_slot`], each `(cx, cy, r²)` for [`sketch_drill`]:
/// a disc inscribed in one arm, a disc circumscribing the whole L, and a disc inside the notch the
/// L does *not* cover.
///
/// Together they are the two-sided differential of AUTH.1e.4, sharpened for a non-convex shape: the
/// developed slot must **contain** the first, **lie within** the second, and be **disjoint** from
/// the third. The exclusion is the one with teeth — a slot silently convexified to its bounding
/// band passes both containments and swallows the notch.
///
/// All three are computed in the L's own `(u, v)` frame: the inscribed disc is centred on the arm's
/// midline at radius `t/2`, the circumscribing one on the reflex corner at the arm's half-diagonal
/// (with a margin, so containment is strict), the notch one at the centre of the removed corner
/// square.
pub fn ell_probes() -> [(Q, Q, Q); 3] {
    [
        (q(7, 80), q(171, 80), q(1, 256)),
        (q(3, 40), q(89, 40), q(3, 80)),
        (q(13, 80), q(179, 80), q(1, 400)),
    ]
}

/// The **Stage-1 flex panel**: the apex cone gore on `σ ∈ [−1, 1]`, four solid cutters with roles
/// derived — D1 the `z ≤ 3` half-space bound, D2 the eccentric apex cylinder, D3 the rim notch,
/// D4 the interior drill.
///
/// The apex cone has a vanishing pedal, so this device develops with `γ ≡ 0` throughout. That
/// makes it the control against the self-lapping cone: a bound that moves on both did not move
/// because of the flat-directrix quadrature.
pub fn flex_panel() -> Part<Bignum> {
    construct::from_chart::<Bignum>(&cone())
        .region_sigma(qi(-1), qi(1), SupportFn::inherit())
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2)))
        .subtract(Cutter::vertical_cylinder(q(-9, 4), q(9, 4), q(9, 16)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(11, 5), q(1, 25)))
        .clearance(qi(1))
}
