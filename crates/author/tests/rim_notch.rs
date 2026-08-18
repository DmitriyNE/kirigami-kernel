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
/// What is asserted is the shape of the answer, not a golden: **`Unresolved`, not `Refuted`** —
/// every *structural* blocker on this device is now a resolution question — and an ε at the DRC
/// threshold rather than an order above it. Certifying it outright still needs #292's chart arm;
/// the 3-D arm cannot close on the drawing's imported rim at any subdivision.
#[test]
fn the_drawing_clears_every_structural_blocker_once_the_ramp_is_off_the_tab() {
    let mut spec = self_lapping_spec();
    spec.ccw.ramp_start = acceptance::lapped::Azimuth::Sigma(Q::new(1, 10));
    spec.ccw.ramp_end = acceptance::lapped::Azimuth::Sigma(Q::new(1, 2));
    spec.inner_profile = Some(acceptance::inner_cut_profile());
    let v = self_lapping_cone_from(&spec, 8, 8, false, None).develop();

    let eps = match &v {
        Verdict::Unresolved(e) => e.clone(),
        other => panic!(
            "no structural refusal should remain, got {}",
            fault(match other {
                Verdict::Verified(_) => Verdict::Unresolved(qi(0)),
                Verdict::Refuted(f) => Verdict::Refuted(*f),
                Verdict::Unresolved(e) => Verdict::Unresolved(e.clone()),
            })
        ),
    };
    // The device's clearance is 7, so the DRC gate is ε < 7/2. Landing *at* the gate says every
    // rail is fitted where its wall is; an ε above it is the gross mis-fit #293 removed.
    assert!(
        eps.cmp(&Q::new(7, 2)) != core::cmp::Ordering::Greater,
        "ε {} should sit at the DRC threshold, not above it",
        export::approx::rat_to_f64(&eps)
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
