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

pub mod lapped;
pub mod measure;

pub use lapped::{
    Azimuth, GapPolicy, LapFault, Lapped, LappedCone, OnTop, RampProfile, SideAngles, lapped_cone,
};

use arrange2d::profile::Profile;
use author::construct;
use author::part::{Cutter, Part, SupportFn};
use develop::cone::DevConfig;
use develop::extrude::{Apex, Frame};
use export::trim::RailFit;
use fixtures::devices::cone;
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
/// window, and three piecewise-support regions ride it — body `[−5/4, 4/7]` at `h ≡ 0`, a
/// smoothstep ramp `[4/7, 1]` climbing `0 → Δ = 1/4`, and a tail plateau `[1, 5/4]` at `h ≡ Δ`.
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

/// The same device carrying **one authored feature** — an arbitrary [`Cutter`], subtracted — the
/// stress variant.
///
/// `None` reproduces [`self_lapping_cone`] exactly, op for op, which is what keeps the pinned
/// device pinned: the VV.1 work budgets, VV.2 ε bounds and VV.3 chord goldens are all measured on
/// the featureless recipe, and a baseline that moves whenever a new fixture is added stops being a
/// baseline.
///
/// The parameter is a whole cutter rather than an `(apex, profile)` pair drawn in one hard-coded
/// plane: a feature's *sketch plane* is as much a placement as its apex is, and [`bat_cutter`] is
/// drawn in a tilted one. Callers that want the `z = 0` plane say
/// `Cutter::extrude(sketch_plane(), apex, profile)`.
///
/// Everything AUTH.1/AUTH.2 has been exercised on so far lives on [`sketch_panel`] — one region,
/// `SupportFn::inherit` (so `γ ≡ 0`), no wrap. This is the hard chart: the extruded footprint is
/// traced over a **nonzero flat directrix**, across a **multi-region** development, on a surface
/// that passes over itself. See [`lap_slot`] for where the feature is placed and why.
pub fn self_lapping_cone_with(
    segments: usize,
    support_panels: usize,
    with_drill: bool,
    feature: Option<Cutter<Bignum>>,
) -> Part<Bignum> {
    self_lapping_cone_from(
        &self_lapping_spec(),
        segments,
        support_panels,
        with_drill,
        feature,
    )
}

/// The device's **ops and resolution knobs over a caller's own recipe**.
///
/// [`self_lapping_cone_with`] is this at [`self_lapping_spec`]. It exists so that a test varying
/// one recipe parameter — a ramp profile, a ramp width — does not have to restate the off-axis
/// inner bound, the clearance, the fit and the budget alongside it. A restated op chain is how a
/// test quietly stops measuring the device.
pub fn self_lapping_cone_from(
    spec: &lapped::LappedCone,
    segments: usize,
    support_panels: usize,
    with_drill: bool,
    feature: Option<Cutter<Bignum>>,
) -> Part<Bignum> {
    let mut part = lapped::lapped_cone(spec)
        .expect("the device recipe is valid")
        .part
        // The DRC keep-out is a length in the part's own unit, so it rides the part's scale. Left
        // where it was it would be a *relatively* tighter budget on a larger part — a silent
        // re-tightening of the acceptance bar disguised as a re-proportioning.
        .clearance(qi(7))
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
        // From [`seam_drill_axis`], not restated beside it: the round-trip check reads that
        // function to fold the holes back onto the *same* cylinder, so a second copy here is a
        // silent way for the two to drift apart — and did, when the annulus was re-proportioned.
        let (cx, cy, r2) = seam_drill_axis();
        part = part.subtract(Cutter::vertical_cylinder(cx, cy, r2));
    }
    if let Some(cutter) = feature {
        part = part.subtract(cutter);
    }
    part
}

/// The self-lapping device **as parameters** — the recipe [`self_lapping_cone_with`] is one point
/// of.
///
/// Every number here used to be written down; all that survives is the ones a product engineer
/// would state — **and since 2026-08-17 they are the physical device's**, in the kernel's unit,
/// the millimetre (`interchange::unit`).
///
/// | quantity | value | where it comes from |
/// |---|---|---|
/// | half-angle β | `sin β = 65/97` → 42.07° | the Pythagorean `(72, 65, 97)`, exact |
/// | stack `t` | `6/25` mm = 240 µm | 4-layer flex, `w ∈ ±120 µm` about the midplane |
/// | ramp step `Δ` | `1/4` mm | **pinned**: certified SHEAR `δ = Δ·cot β = Δ·72/65 = 18/65` |
/// | seam gap `g` | `1/100` mm = 10 µm | `Δ − t`; the ACF bondline, since `SEP ≡ ACF gap` |
/// | annulus | Ø 8 → Ø 21.5 mm | the trimmed sheet, cut **normal to itself** at both radii |
/// | ramp width | ≈ 61° of azimuth | the ≈60° degree-1 seam ramp |
///
/// The seam centreline sits `t/2 + g/2 = 1/8` off the base sheet's mid-surface — the value at
/// which one ramp vanishes exactly, so the clockwise end never leaves the base cone and the device
/// has a single ramp. The supports `0`, `0 → 1/4`, `1/4` are *derived* from those three numbers
/// rather than authored, and the ramp's height is `Δ` by construction.
///
/// The azimuths are given in σ because they must be: on the wrapping chart `σ = tan(φ/4)`, so no
/// rational direction names `±5/4`, and this is the spelling that keeps the re-expression exact —
/// which the VV.1 budgets, VV.2 ε bounds and VV.3 goldens all depend on. `ramp_start = 4/7` is
/// `φ = 118.98°`, so the ramp spans `61.02°` — the nearest small rational to the authored 60°.
///
/// **Placing a feature by azimuth.** The kept sheet is the *lower* nappe, so a point of material at
/// chart parameter σ sits at plan azimuth
///
/// ```text
/// az = 270° + 4·arctan σ        (mod 360°)
/// ```
///
/// — which runs `64.65° → 115.35°` the long way round over `σ ∈ [−5/4, 5/4]`, 410.7° in all. So
/// `az ∈ (64.65°, 115.35°)` is the **lap wedge**, swept twice: once by the base cone at `σ < 0` and
/// once by the ramp and tail flap at `σ > 0`. Both cut files draw their feature inside it, which is
/// why each of them appears twice in the flat pattern and why only the `σ > 0` pass can meet a ramp.
/// Note the *ruling direction*'s azimuth is this less 180°: it is the sign of µ̂ that decides which.
///
/// The pick is **derived** now: both trim radii live in the recipe, so `lapped_cone`'s own
/// mid-annulus point at `ρ = (4 + 43/4)/2 = 59/8` is in material by construction. It had to be
/// named while the inner bound was an off-axis cylinder applied afterwards, which the recipe could
/// not see.
pub fn self_lapping_spec() -> lapped::LappedCone {
    let sigma = |n: i128, d: i128| lapped::Azimuth::Sigma(q(n, d));
    lapped::LappedCone {
        // The Pythagorean (72, 65, 97): sin β = 65/97, the 42° device, exact.
        apex: (qi(72), qi(65)),
        thickness: q(6, 25),
        gap: q(1, 100),
        on_top: lapped::OnTop::Ccw,
        seam_offset: q(1, 8),
        ccw: lapped::SideAngles {
            ramp_start: sigma(4, 7),
            ramp_end: sigma(1, 1),
            sheet_end: sigma(5, 4),
        },
        cw: lapped::SideAngles::flat(sigma(-5, 4)),
        outer_r: q(43, 4),
        // **Not yet the drawing**, for the same reason the bore is not: `outer_cut_profile()` is the
        // rim this device is meant to have — the Ø 21.5 circle interrupted by a lug over 15° about
        // `+y` — and the file reads exactly. Where the lug lands is what decides whether it places:
        // its wedge sits inside the lap, so the chart passes it twice, and only a pass with `h′ ≠ 0`
        // can run a ruling across a radial flank. Left `None` so the pinned device stays the object
        // every V&V number was taken on; `author/tests/rim_notch.rs` is where it is measured.
        outer_profile: None,
        inner_r: Some(qi(4)),
        // **Not yet the drawing.** `inner_cut_profile()` is the bore this device is meant to have —
        // the file reads exactly and the recipe carries it. What the resolver cannot yet place is
        // the tab, and only where it crosses the **ramp**: there the ruling's plan projection
        // misses the axis by up to 0.481 mm against a tab 0.35 mm half-wide, so one ruling runs
        // across the tab instead of along it and the kept material becomes two µ̂-intervals at one
        // σ — `PartFault::SectionNotSimple`, the one-interval boundary model's own frontier. On the
        // base cone (`h′ = 0`, plan miss exactly 0) the same tab certifies, which is the controlled
        // half of the measurement: the drawing's tab passes the chart twice and only the ramp pass
        // splits. Task #291 carries it; `author/tests/rim_notch.rs` pins it. Left `None` so the
        // pinned device stays the object every V&V number was taken on, rather than a refusal.
        inner_profile: None,
        // The physical edge: cut **normal to the sheet**, not vertically. Both bounds are cones,
        // and both are recognized as cones of revolution (`develop::cut::RevCone`), so they carry
        // the same closed-form distance a cylinder does and cost the same split.
        trim: lapped::TrimStyle::NormalCut,
        // The bending-neutral mid-plane: what `seam_offset` is measured against.
        neutral: q(1, 2),
        // The cubic, because every pinned measurement on this device was taken on it.
        // `EvenCurvature` halves nothing here but the fold-line swing; see `RampProfile`.
        ramp_profile: lapped::RampProfile::Smoothstep,
        // The ramp deliberately descends *inside* the lap here, so the gap closes over part of the
        // seam and `Constant` would refuse it. What it actually reaches is BONDED's to report.
        policy: lapped::GapPolicy::MinDistance,
        pick: None,
    }
}

/// **The two-ramp device, carrying both drawings** — the same 42° cone, stack and annulus as
/// [`self_lapping_spec`], with the seam centred on the base sheet instead of offset onto one side
/// of it.
///
/// One number differs, `seam_offset = −1/8` against `+1/8`, and it costs a ramp on each side:
/// the pinned device's clockwise end never leaves the base cone, this one's does. That is what a
/// board wants when neither end may bulge more than the other — and it is also the configuration
/// that can carry **both** cut files at once, which the pinned device cannot.
///
/// Why this one places them: its ramps sit at `σ ∈ [37/80, 7/8]`, so each drawing's wedge meets
/// `h′ = 0` sheet on both of its passes, where a ruling's plan projection runs through the axis and
/// *along* a radial flank rather than across it. On the pinned device the second pass lands on the
/// ramp and the bore refuses `SectionNotSimple` (#291), which is why `self_lapping_spec` leaves
/// both profiles `None`.
///
/// Both ramps span `Δσ = 7/20`. A ramp's edge of regression sweeps `≈ 0.9·h/Δσ²` along the ruling
/// and must stay inside the inner bound's `µ̂ ≈ 1.81` or the sheet would have to crease:
/// `0.9·(1/8)/(7/20)² ≈ 0.92`, clear by about 2×. Narrow them and the recipe is refused, soundly.
///
/// Lives here rather than in the driver that runs it because `author/tests/lapped_cone.rs` must
/// certify the *same* device the demo emits. A test that restated these numbers would keep passing
/// while the two drifted apart — the failure the `acceptance` crate exists to prevent.
pub fn two_ramp_spec() -> lapped::LappedCone {
    let sigma = |n: i128, d: i128| lapped::Azimuth::Sigma(q(n, d));
    lapped::LappedCone {
        // The Pythagorean (72, 65, 97): sin β = 65/97, the same 42° device.
        apex: (qi(72), qi(65)),
        thickness: q(6, 25),
        gap: q(1, 100),
        on_top: lapped::OnTop::Cw,
        // c = 0 in physical terms: the seam straddles the base cone, so BOTH ends ramp, by
        // ∓(t/2 + g/2) = ∓1/8.
        seam_offset: q(-25, 200),
        ccw: lapped::SideAngles {
            ramp_start: sigma(37, 80),
            ramp_end: sigma(7, 8),
            sheet_end: sigma(9, 8),
        },
        cw: lapped::SideAngles::flat(sigma(-9, 8)),
        outer_r: q(43, 4),
        // The device's real rim: the Ø 21.5 circle with a lug reaching out to Ø 27.5 on two radial
        // flanks and a 195° nose arc tangent to both.
        outer_profile: Some(outer_cut_profile()),
        inner_r: Some(qi(4)),
        // The device's real bore: the Ø 8 hole with a 10° tab reaching in to Ø 4.
        inner_profile: Some(inner_cut_profile()),
        // Both bounds are cones cut normal to the sheet — a cone of revolution has a closed-form
        // distance, so `develop::cut::RevCone` puts them on the same certificate arm a cylinder is
        // on and they cost the same.
        trim: lapped::TrimStyle::NormalCut,
        neutral: q(1, 2),
        // `h''` constant in magnitude, so the bend spreads across the ramp instead of piling up at
        // its two joins: measured 1.5× less fold-line swing.
        ramp_profile: lapped::RampProfile::EvenCurvature,
        // Both ramps finish before the overlap starts (ramp_end 7/8 against the lap's 8/9), so the
        // gap really is `g` across the whole seam and the strict policy holds.
        policy: lapped::GapPolicy::Constant,
        pick: None,
    }
}

/// **The device's inner cut, as the drawing states it** — `data/inner-cut.dxf`, verbatim.
///
/// Embedded rather than read from disk so the device is the same object wherever it is built, and
/// kept as *text* so that the file, not a transcription of it, is the definition. Everything that
/// consumes the device — the demos, the V&V pins, the benchmark, `tests/imported_outline.rs` —
/// reads this one string.
pub const INNER_CUT_DXF: &str = include_str!("../data/inner-cut.dxf");

/// The unit [`INNER_CUT_DXF`] is read in.
///
/// The file carries `$MEASUREMENT 1` (metric) but no `$INSUNITS`, and those are different claims:
/// `$MEASUREMENT` picks a linetype table, it does not say millimetre rather than centimetre. So the
/// reader refuses to infer — an inferred unit is a 10× or 25.4× part — and the unit is supplied
/// here, where it is a statement about *this* drawing rather than a default.
pub const INNER_CUT_UNIT: interchange::unit::Unit = interchange::unit::Unit::Millimetre;

/// **The device's outer trim, as the drawing states it** — `data/outer-cut.dxf`, verbatim.
///
/// Embedded for the same reason [`INNER_CUT_DXF`] is: the file, not a transcription of it, is the
/// definition, and the device is the same object wherever it is built.
pub const OUTER_CUT_DXF: &str = include_str!("../data/outer-cut.dxf");

/// The unit [`OUTER_CUT_DXF`] is read in — the same drawing, the same statement as
/// [`INNER_CUT_UNIT`]: `$MEASUREMENT 1` picks a linetype table, it does not say millimetre.
pub const OUTER_CUT_UNIT: interchange::unit::Unit = interchange::unit::Unit::Millimetre;

/// The outer trim's outline, read out of [`OUTER_CUT_DXF`].
///
/// Four entities on layer `VISIBLE` forming one closed loop: the Ø 21.5 rim, interrupted over a
/// 15° wedge about `+y` by a **lug** — two exactly radial flanks running out from the rim to a
/// 195° nose arc tangent to both, tipped at ρ = 13.75. So it is [`inner_cut_profile`]'s tab
/// inverted: material reaching *out* of the rim rather than *in* to the bore, drawn with the same
/// two wall kinds.
///
/// The flanks being radial is geometry, not drafting taste. Cast from a point on the axis
/// ([`lapped::TrimStyle::NormalCut`]) a radial sketch line sweeps a **plane through the axis**, so the lug's
/// flanks are the same walls the bore's tab has — which is what makes this outline the outer
/// counterpart rather than a second unrelated shape.
///
/// Panics only on a file this repository ships, so a failure is a broken commit rather than a
/// runtime condition; `tests/imported_outline.rs` is what names it.
pub fn outer_cut_profile() -> Vec<Edge<Bignum>> {
    let opts = interchange::dxf::DxfOptions::<Bignum> {
        assume_unit: Some(OUTER_CUT_UNIT),
        ..Default::default()
    };
    interchange::dxf::read_dxf::<Bignum>(OUTER_CUT_DXF, &opts)
        .expect("data/outer-cut.dxf is a readable outline")
        .profile()
        .into_edges()
}

/// The inner cut's outline, read out of [`INNER_CUT_DXF`].
///
/// Eight `ARC`/`LINE` entities on layer `VISIBLE` forming one closed loop: the Ø 8 hole with a 10°
/// tab reaching in to Ø 4, filleted R 0.25 at the root and R 0.15 at the tip. Four of its junctions
/// are **arc to arc**, which is the case `interchange::element` had written down and left unbuilt
/// until a real file needed it.
///
/// The read is exact to `δ = 2.6e-14` — the drawing's own `ARC` entities over-determine their
/// endpoints, and the junction re-gauge charges what it moves — over a closure gap of `2.3e-10`,
/// which is the file's own sloppiness and lands entirely on the two `LINE` endpoints, where moving
/// costs nothing.
///
/// Panics only on a file this repository ships, so a failure is a broken commit rather than a
/// runtime condition; `tests/imported_outline.rs` is what names it.
pub fn inner_cut_profile() -> Vec<Edge<Bignum>> {
    let opts = interchange::dxf::DxfOptions::<Bignum> {
        assume_unit: Some(INNER_CUT_UNIT),
        ..Default::default()
    };
    interchange::dxf::read_dxf::<Bignum>(INNER_CUT_DXF, &opts)
        .expect("data/inner-cut.dxf is a readable outline")
        .profile()
        .into_edges()
}

/// **The battery window, as the drawing states it** — `data/bat-cutout.dxf`, verbatim.
///
/// Embedded for the same reason [`INNER_CUT_DXF`] is: the file, not a transcription of it, is the
/// definition.
pub const BAT_CUTOUT_DXF: &str = include_str!("../data/bat-cutout.dxf");

/// The unit [`BAT_CUTOUT_DXF`] is read in — the same statement as [`INNER_CUT_UNIT`]: the file
/// carries `$MEASUREMENT 1`, which picks a linetype table and does not name a length unit.
pub const BAT_CUTOUT_UNIT: interchange::unit::Unit = interchange::unit::Unit::Millimetre;

/// The battery window's outline, read out of [`BAT_CUTOUT_DXF`].
///
/// Four `LINE` entities on layer `VISIBLE` forming one closed rectangle, `4.6 × 2.7` mm, centred on
/// the sketch's `y` axis with its near edge `6.4198` from the sketch origin. Every wall is affine,
/// so each casts to a **plane** and the whole footprint certifies at `ε = 0` — unlike the bore and
/// the rim, whose arcs cast to quadrics.
///
/// Panics only on a file this repository ships, so a failure is a broken commit rather than a
/// runtime condition.
pub fn bat_cutout_profile() -> Vec<Edge<Bignum>> {
    let opts = interchange::dxf::DxfOptions::<Bignum> {
        assume_unit: Some(BAT_CUTOUT_UNIT),
        ..Default::default()
    };
    interchange::dxf::read_dxf::<Bignum>(BAT_CUTOUT_DXF, &opts)
        .expect("data/bat-cutout.dxf is a readable outline")
        .profile()
        .into_edges()
}

/// **The battery window's sketch plane** — through the cone's apex, `x` aligned with the world's,
/// `y` tilted down over the material.
///
/// The tilt is stated the way the cone's own half-angle is, **from the axis**: the plane's line of
/// steepest descent runs `44.0725°` off the axis against the cone's `42.0750°`, so the plane clears
/// the surface everywhere but the apex and leans over the sheet by exactly `2°`. That two-degree
/// stand-off is what makes the sweep direction a near-normal cut rather than a raking one.
///
/// `sin` and `cos` are **exact**: `(1428, 1475, 2053)` is Pythagorean, so the frame is orthonormal
/// ([`Frame::metric`](develop::extrude::Frame::metric)) and the drawing's millimetres survive the
/// placement — an affine frame would stretch the window's `2.7` mm height by `|v|`. The nearest
/// rational unit vector to the authored angle is `0.0025°` away, which is the price of exactness and
/// is stated rather than hidden.
pub fn bat_plane() -> Frame<Bignum> {
    Frame::new(
        [qi(0), qi(0), qi(0)],
        [qi(1), qi(0), qi(0)],
        [qi(0), q(1428, 2053), q(-1475, 2053)],
    )
    .expect("the axes are independent")
}

/// The battery window's **sweep direction** — the [`bat_plane`]'s own normal, `u × v`.
///
/// A direction rather than a point, so the cutter is a straight prism: through all, no draft, walls
/// perpendicular to the sketch plane. `Apex::direction` takes it projectively, so the integer
/// `(0, 1475, 1428)` needs no normalizing and the prism runs both ways from the sketch.
pub fn bat_sweep() -> Apex<Bignum> {
    Apex::direction([qi(0), qi(1475), qi(1428)]).expect("a real sweep direction")
}

/// The battery window as a **cutter**: [`bat_cutout_profile`] drawn in [`bat_plane`], swept along
/// [`bat_sweep`].
pub fn bat_cutter() -> Cutter<Bignum> {
    Cutter::extrude(bat_plane(), bat_sweep(), bat_cutout_profile())
}

/// The centre of the self-lapping device's seam drill, `(x, y, r²)` — the 3-D cylinder both
/// derived holes must fold back onto. Exposed so a round-trip check tests the *same* cylinder the
/// part was cut with instead of restating its numbers.
///
/// The **direction** is the design choice — `(−7, 25)` puts it on the lapped wedge at `az ≈ 100.6°`,
/// and azimuth is what fixes which σ, which region and which ramp height the hole lands at. The
/// **radius** only has to be in the annulus, and this one sits mid-way: `ρ = 7.63` in `[4, 10.75]`.
/// So a re-proportioning scales this vector and leaves every σ-pinned measurement alone, which is
/// exactly what the Ø 43 → Ø 21.5 correction did (`×3/5`).
pub fn seam_drill_axis() -> (Q, Q, Q) {
    (q(-7, 5), q(15, 2), q(9, 16))
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
/// * **On the ramp.** The far sheet lands strictly inside the smoothstep band `[4/7, 1]` — the
///   test probes it at `σ = 7/8`, where `|γ| = 0.477` — so that hole is traced over a **nonzero
///   flat directrix**, while its twin on the body is traced at `γ ≡ 0` (probed at `σ = −1/2`). One
///   cutter, both development tiers, and the difference between the two holes is attributable to
///   `γ` and to nothing else. `self_lapping_slot.rs` pins the far hole's normal offset to
///   `0.6 Δ … 0.98 Δ`, i.e. on the ramp and hugging neither end of it.
/// * **Clear of the joins.** Neither footprint crosses a region boundary, which is refused
///   ([`PartFault::HoleCrossesRegions`](author::part::PartFault::HoleCrossesRegions)) rather than
///   realized.
/// * **Inside the annulus.** Its radii sit around `ρ ≈ 7.6`, between the inner bound at `4` and the
///   outer at `10.75`, so it is an interior hole and not a rim bite. Like the seam drill it is
///   placed by *direction*, and re-proportioning scales the vector rather than re-authoring it.
///
/// The axes are turned for the same reason [`ell_slot`]'s are: the rulings project to radial rays,
/// so an L whose arms lie along the radius is met once and its footprint is an ordinary band. Here
/// the local radial direction resolves to `(−0.63, +0.78)` in `(u, v)` — opposite signs, so a ray
/// leaves one arm, crosses the notch and re-enters the other. `(3, 4, 5)` keeps every vertex exact.
pub fn lap_slot() -> Vec<Edge<Bignum>> {
    let (cx, cy, a, t) = (q(7, 5), q(15, 2), q(7, 6), q(7, 12));
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
/// ruling (`SectionNotSimple`, §12.5).
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

/// The **contour panel**'s authored outline, `(cx, cy, w, h, r)` — the numbers
/// [`contour_panel`] is cut with. Exposed so a faithfulness check tests the *same* rounded
/// rectangle the part was built from instead of restating it (the [`seam_drill_axis`] doctrine).
pub fn contour_outline_geometry() -> (Q, Q, Q, Q, Q) {
    (qi(0), q(11, 5), q(1, 4), q(1, 5), q(1, 10))
}

/// The **contour panel**: the Stage-1 cone gore whose boundary is an **authored outline** rather
/// than a declared σ-band — the σ-stock (AUTH.3) as a product part.
///
/// Every other device here is a *band*: `region_sigma` says where the material starts and stops,
/// and cutters only trim µ̂. This one keeps what is inside a radiused rectangle
/// ([`rounded_outline`]), so its σ-extent is **derived** from the contour's own corners. That is
/// what a flex circuit's boundary actually is — a closed outline drawn in ECAD — and it is the one
/// thing `intersect` could not express before AUTH.3.
///
/// The panel's own `z ≤ 3` bound and annulus carve stay in the recipe and both resolve
/// **`Inactive`**: the outline is small and sits clear of them, so it bounds the part *alone*. That
/// is deliberate — it makes "the contour is the whole boundary" a derived fact the report states,
/// not a property of a recipe pruned to force it.
///
/// `feature` is an interior cut authored in **flat** (ECAD) coordinates and folded back onto the
/// surface, which is the round-trip leg: the outline goes 3-D → flat, the feature goes flat → 3-D,
/// and the two meet in one part. Its coordinates depend on where the development lands, so a caller
/// develops once with `None` and places it from the flat pattern it gets.
///
/// **The op is well-posed here and would not be on the wrapping device** (`§12.5`): a ruling is a
/// line through the apex and a swept profile is a prism, so keeping what is inside a contour is
/// meaningful only where no azimuth *and its antipode* are both swept. This gore spans 180° at
/// `σ ∈ [−1, 1]`; the self-lapping cone's 410.7° does not qualify, and refuses by name.
pub fn contour_panel(segments: usize, feature: Option<Vec<[Q; 2]>>) -> Part<Bignum> {
    let (cx, cy, w, h, r) = contour_outline_geometry();
    let part = construct::from_chart::<Bignum>(&cone())
        .region_sigma(qi(-1), qi(1), SupportFn::inherit())
        .keep_near(
            cone()
                .surface(&qi(2), &qi(0))
                .eval(&qi(0))
                .expect("the cone is regular at σ = 0"),
        )
        .intersect(Cutter::half_space([qi(0), qi(0), qi(1)], qi(3)))
        .subtract(Cutter::vertical_cylinder(qi(0), q(1, 2), qi(2)))
        .intersect(Cutter::extrude(
            sketch_plane(),
            Apex::direction([qi(0), qi(0), qi(1)]).expect("a real sweep direction"),
            rounded_outline(cx, cy, w, h, r),
        ))
        .clearance(qi(1))
        .thickness(q(1, 8))
        .segments(segments);
    match feature {
        Some(poly) => part.hole_flat(poly),
        None => part,
    }
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
