//! **#291 — a tab in the bore splits the material's µ̂-section, and the refusal says so.**
//!
//! The device's real inner cut (`acceptance/data/inner-cut.dxf`) is the Ø 8 bore with a 10° tab
//! reaching in to Ø 4. Cutting the device with it is refused, and *which* refusal it earns is the
//! whole content of these tests: the resolver models a region as **one µ̂-interval per σ** — a
//! lower rail and an upper rail, both graphs over σ — plus interior holes, and a tab that a ruling
//! crosses sideways makes the material two intervals at one σ. That is
//! [`PartFault::SectionNotSimple`], not an ambiguity and not a degenerate wall.
//!
//! **Measured on the pinned device, and this is why the name matters.** The tab appears *twice* on
//! the 410.7° chart, at the same plan azimuth:
//!
//! | pass | region | `h` | ruling's plan miss | section |
//! |---|---|---|---|---|
//! | σ ≈ −1.079…−0.927 | 0 | `0` | **exactly 0** (radial) | one interval ✓ |
//! | σ ≈ +0.888…+1.049 | 1 (the ramp) | `0 → 1/4` | up to **0.481 mm** | **two intervals** at σ = 0.897, 0.906, 0.915 ✗ |
//!
//! Same cut, same azimuth, same walls traversed in the same order — only the sheet differs. The
//! split stretches are separated by a gap running from the tab's root fillet to its flank, and it
//! is *not* a hole: it opens into the exterior at the low-σ end of its band, so merging the two
//! stretches into a face-with-hole would emit a closed island where the part has an open bay.
//!
//! The tab is 0.35 mm half-wide at its root and the ramp's rulings miss the axis by more than that,
//! which is the one-line statement of the geometry: on the ramp the ruling is wider off-axis than
//! the tab is wide.

use acceptance::{self_lapping_cone_from, self_lapping_spec};
use arrange2d::profile::Profile;
use author::part::{Part, PartFault};
use certify_core::Verdict;
use geom::content::Edge;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// The device carrying `profile` as its inner cut, at the demo's own resolution knobs.
fn device_with(profile: Vec<Edge<Bignum>>) -> Part<Bignum> {
    let mut spec = self_lapping_spec();
    spec.inner_profile = Some(profile);
    self_lapping_cone_from(&spec, 8, 8, false, None)
}

fn fault(v: Verdict<author::part::FlatPattern<Bignum>, PartFault, Q>) -> String {
    match v {
        Verdict::Verified(_) => "Verified".to_string(),
        Verdict::Unresolved(_) => "Unresolved".to_string(),
        Verdict::Refuted(f) => format!("{f:?}"),
    }
}

/// **The drawing itself.** `data/inner-cut.dxf` reads exactly and the recipe carries it; what the
/// resolver cannot yet place is the tab, and it must say which of its refusals this is.
///
/// Op 1 is the inner cut — the op whose gap splits the section. Op 0 is the outer trim.
#[test]
fn the_device_drawing_splits_the_material_section_and_is_refused_by_name() {
    let v = device_with(acceptance::inner_cut_profile()).develop();
    assert!(
        matches!(v, Verdict::Refuted(PartFault::SectionNotSimple { op: 1 })),
        "the drawing's tab must refuse as a split section on op 1, got {}",
        fault(v)
    );
}

/// **#293 — a wall that bounds twice is fitted twice.** Move the ccw ramp off the tab and the
/// section stops splitting, so the drawing gets past [`PartFault::SectionNotSimple`] and the *next*
/// thing it meets is the rail fitting. Two fields differ from the pinned device, which is what
/// keeps this a measurement of that device rather than a restated one.
///
/// The tab's root fillet bounds the material in **two disjoint σ-runs** — the 410.7° chart passes
/// its azimuth twice — and a single hull over both spanned 60°+ of azimuth for a fillet subtending
/// 7.6°, on which the float oracle rightly declined (`disc < 0`). One rail per run fixes it.
///
/// **Where the drawing now stops, and it is three fixes further on.** The chain, each step pinned
/// by a measurement rather than inferred:
///
/// | | the drawing reported |
/// |---|---|
/// | before any of this | `Refuted(NappeCrossed)` — a DRC cushion used as a soundness gate |
/// | after the per-ball nappe check | `Unresolved ε 1.1220e1` |
/// | after #293's rail-per-run | `Unresolved ε 3.5000e0` — exactly `clearance/2`, the ball bound's "nothing certified" sentinel on eight tab rails |
/// | after the centred discriminant | **`Refuted(RailSpanShort)`** |
///
/// The last step is the interesting one and the reason `Refuted` here is *progress*, not
/// regression: [`PartFault::RailSpanShort`] is only reachable once a rail **has** a certificate and
/// the boundary needs it over σ the certificate does not cover. The eight rails on the tab's
/// sub-millimetre walls — R 0.25 root fillets, R 0.15 tip fillets, the flanks — could not be
/// certified *at all* while the µ̂-discriminant's enclosure straddled zero. They can now, and what
/// is left is §12.4's p-curve end: a graph rail cannot reach a tangent ruling, and the turn arc has
/// to own that stretch.
#[test]
fn the_drawings_tab_rails_certify_and_what_is_left_is_the_p_curve_end() {
    let mut spec = self_lapping_spec();
    spec.ccw.ramp_start = acceptance::lapped::Azimuth::Sigma(Q::new(1, 10));
    spec.ccw.ramp_end = acceptance::lapped::Azimuth::Sigma(Q::new(1, 2));
    spec.inner_profile = Some(acceptance::inner_cut_profile());
    let v = self_lapping_cone_from(&spec, 8, 8, false, None).develop();

    assert!(
        matches!(v, Verdict::Refuted(PartFault::RailSpanShort { op: 1 })),
        "the tab's rails should certify and leave the p-curve end, got {}",
        fault(v)
    );
}

/// **The flank, not the sheet, on its own is enough.** A straight-sided tab — vertical flanks, a
/// chord tip — is crossed sideways by a *radial* ruling too, because its flanks are not radial. So
/// this one splits the section on the base cone, where the drawing's radial-flanked tab does not.
///
/// Widening it does not help and that is the point: the two-interval band lives between the tab's
/// corner azimuth and its flank azimuth, which every width has. Measured across half-widths 0.347
/// through 2.400 mm, all refused.
#[test]
fn a_straight_flanked_tab_splits_the_section_at_every_width() {
    // A Pythagorean (23, 264, 265) puts both top corners exactly on r = 4: half-width 92/265.
    let (w, ytop) = (Q::new(92, 265), Q::new(1056, 265));
    let (a, b) = ([w.clone(), ytop.clone()], [w.neg(), ytop]);
    let tab = Profile::new()
        .arc(qi(0), qi(0), qi(16), b.clone(), a.clone())
        .polyline(&[a, [w.clone(), qi(2)], [w.neg(), qi(2)], b])
        .into_edges();

    let v = device_with(tab).develop();
    assert!(
        matches!(v, Verdict::Refuted(PartFault::SectionNotSimple { op: 1 })),
        "a straight-flanked tab must refuse as a split section on op 1, got {}",
        fault(v)
    );
}
