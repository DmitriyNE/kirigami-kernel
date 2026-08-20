//! **Normal-cut trims** — an annulus bounded by cuts made perpendicular to the sheet.
//!
//! A vertical cylinder meets a 42° cone at a bevel; a real trim is cut normal to the surface. That
//! boundary is a cone too, and it needs no new cutter kind — a disc swept from an apex *is* one:
//!
//! 1. put the disc's plane at the `z` where the base cone's **neutral surface** has radius `r` —
//!    on `72ρ + 65z = 0`, that is `z = −(72/65)·r`;
//! 2. put the apex on the axis so the generatrix through the rim runs along the cone's own normal
//!    `(72, 65)/97`, i.e. `z_apex = z_r − (65/72)·r = −(97²/(65·72))·r`.
//!
//! Both come out with generatrix ratio `Δρ/Δz = 72/65` exactly — half-angle `90° − β`, as a normal
//! cut must be — and every number is rational.
//!
//! Two things had to be true for this to certify, and each was a distinct defect:
//!
//! * the resolver labels a wall end by which side of the cutter's **shadow** it is, while the
//!   oracle picks a **root** of the µ̂-quadratic, and the two only agree when that quadratic opens
//!   upward (`author::realize`'s `mu_form_opens_up`) — the two branches below are what that is
//!   about;
//! * a cone of revolution has a **closed-form distance**, so its certificate is a symbolic residual
//!   in σ like a cylinder's, not a first-order bound inside an inflated ball
//!   (`develop::cut::RevCone`). Before that the same cut wanted 64× the split and still could not
//!   clear its own apex on the device.

use arrange2d::profile::Profile;
use author::part::{Cutter, Part, SupportFn};
use certify_core::Verdict;
use develop::cone::DevConfig;
use develop::cut::CutSurface;
use develop::extrude::{Apex, Frame, ellipse_wall};
use export::approx::rat_to_f64;
use export::cut_oracle::RootPick;
use export::trim::{RailFit, certified_rail_surface};
use fixtures::devices::cone_wrap;
use lattice::{Bignum, Interval, Rat, RatFunc};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// The `z` of the neutral surface's circle of radius `r`, and the apex that casts it along the
/// cone's own normal.
fn normal_cast(r: &Q) -> (Q, Q) {
    let z_r = r.mul(&q(-72, 65));
    (z_r.clone(), z_r.sub(&r.mul(&q(65, 72))))
}

/// The normal-cut cone through the neutral surface's circle of radius `r`, as a cutter.
fn normal_cone(r: &Q) -> Cutter<Bignum> {
    let (z_r, z_apex) = normal_cast(r);
    Cutter::extrude(
        Frame::new(
            [qi(0), qi(0), z_r],
            [qi(1), qi(0), qi(0)],
            [qi(0), qi(1), qi(0)],
        )
        .expect("the xy axes are independent"),
        Apex::point([qi(0), qi(0), z_apex]),
        Profile::new().circle(qi(0), qi(0), r.clone()).into_edges(),
    )
}

/// The same cone as a bare cut surface — the wall the cutter's single arc sweeps.
fn normal_cone_wall(r: &Q) -> CutSurface<Bignum> {
    let (z_r, z_apex) = normal_cast(r);
    ellipse_wall(
        &[qi(0), qi(0), z_r],
        &[r.clone(), qi(0), qi(0)],
        &[qi(0), r.clone(), qi(0)],
        &Apex::point([qi(0), qi(0), z_apex]),
    )
    .expect("a real cone")
}

/// The coaxial vertical cylinder of the same radius.
fn cylinder_wall(r: &Q) -> CutSurface<Bignum> {
    CutSurface::Cylinder {
        axis_point: [qi(0), qi(0), qi(0)],
        axis_dir: [qi(0), qi(0), qi(1)],
        r2: r.mul(r),
    }
}

/// The gore, outer-bounded by a cylinder at `r = 5`, inner-bounded by `inner`.
fn gore(inner: Cutter<Bignum>, subdiv: usize) -> Part<Bignum> {
    let rz0 = cone_wrap()
        .ruling()
        .comp(2)
        .eval(&qi(0))
        .expect("the wrap chart's ruling is regular at σ = 0");
    let witness = cone_wrap()
        .surface(&q(-54, 13).div(&rz0), &qi(0)) // z = −(72/65)·(15/4): mid-annulus
        .eval(&qi(0))
        .expect("the witness is regular");
    author::construct::from_chart::<Bignum>(&cone_wrap())
        .region_sigma(q(-5, 4), q(5, 4), SupportFn::constant(qi(0)))
        .keep_near(witness)
        .intersect(Cutter::vertical_cylinder(qi(0), qi(0), qi(25)))
        .thickness(q(6, 25))
        .subtract(inner)
        .clearance(qi(1))
        .fit(RailFit {
            degree: 4,
            subdiv,
            bits: 44,
        })
        .segments(64)
        .support_panels(8)
        .budget(DevConfig {
            terms: 14,
            sqrt_eps: q(1, 1_000_000_000),
        })
}

/// The gore's σ-span.
fn span() -> Interval<Bignum> {
    Interval {
        lo: q(-5, 4),
        hi: q(5, 4),
    }
}

/// Certify one wall's rail over the gore span at `subdiv`, or panic with the verdict.
fn rail(surface: &CutSurface<Bignum>, subdiv: usize) -> (RatFunc<Bignum>, Q) {
    let cfg = DevConfig {
        terms: 14,
        sqrt_eps: q(1, 1_000_000_000),
    };
    let fit = RailFit {
        degree: 4,
        subdiv,
        bits: 44,
    };
    match certified_rail_surface(
        &cone_wrap(),
        surface,
        RootPick::Upper,
        &span(),
        fit,
        (false, false),
        &qi(1),
        &cfg,
    ) {
        Verdict::Verified(x) => x,
        Verdict::Unresolved(e) => panic!("unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
        Verdict::Refuted(f) => panic!("refuted: {f:?}"),
    }
}

/// **A normal cut and a vertical cut at the same radius certify to the same ε, at the same split.**
///
/// They meet the sheet in the *same circle* — the base cone and the cutter cone are coaxial
/// surfaces of revolution — so this is one geometric fact reached through two unrelated surface
/// representations: a `CutSurface::Cylinder` with `|√perp2 − R|`, and a `CutSurface::Quadric`
/// recognized as a cone of revolution and measured by the drop onto its generatrix. Agreement to
/// the digit is the corroboration.
///
/// Both run at `subdiv = 160`. The quadric route used to need 64× that, because its arm enclosed
/// the traced point in a box instead of cancelling the surface equation against the chart fields;
/// recognizing the cone is what removed the difference rather than paying it down.
#[test]
fn a_normal_cut_and_a_vertical_cut_agree_at_the_same_radius() {
    let eps = |p: Part<Bignum>| -> f64 {
        match p.develop() {
            Verdict::Verified(f) => rat_to_f64(f.eps()),
            Verdict::Refuted(f) => panic!("refuted: {f:?}"),
            Verdict::Unresolved(e) => panic!("unresolved at ε ≈ {:.3e}", rat_to_f64(&e)),
        }
    };
    let cyl = eps(gore(Cutter::vertical_cylinder(qi(0), qi(0), q(25, 4)), 160));
    let cone = eps(gore(normal_cone(&q(5, 2)), 160));
    assert!(
        (cyl - cone).abs() < 1e-12,
        "the same circle, two surface representations: {cyl:.6e} vs {cone:.6e}"
    );
}

/// **The two rails are the same circle, reached by opposite branches — which is the whole trap.**
///
/// Certified against the cylinder and against the cone, `RootPick::Upper` returns rails that are
/// exact negatives of one another: `+3.2199…` against `−3.2199…` at the span's end. Both are
/// truthful — each really does lie on the surface it was certified against, to `< 1e-7` — because
/// the cone's µ̂-quadratic **opens downward**, so its upper root is the sheet's *other* side.
///
/// That is the geometry behind `author::realize`'s `mu_form_opens_up`, stated where it can be seen:
/// a resolver that hands `Upper` to the oracle because the shadow's upper end is wanted gets a rail
/// on the far side of the cone, at full radius, perfectly certified, and completely wrong. Matching
/// magnitudes are the evidence the two surfaces are the same circle; opposite signs are the
/// evidence that the label alone does not say which side of it.
#[test]
fn the_two_rails_are_one_circle_reached_by_opposite_branches() {
    let r = q(5, 2);
    let (cyl, e_cyl) = rail(&cylinder_wall(&r), 160);
    let (cone, e_cone) = rail(&normal_cone_wall(&r), 160);
    let tol = q(1, 10_000_000);
    assert!(
        e_cyl.cmp(&tol) == core::cmp::Ordering::Less
            && e_cone.cmp(&tol) == core::cmp::Ordering::Less,
        "both rails are on their own surface: ε {:.3e} and {:.3e}",
        rat_to_f64(&e_cyl),
        rat_to_f64(&e_cone)
    );
    for k in -5i128..=5 {
        let s = q(k, 4);
        let a = cyl
            .eval(&s)
            .expect("the cylinder rail is regular on the span");
        let b = cone.eval(&s).expect("the cone rail is regular on the span");
        assert!(
            a.sign() > 0 && b.sign() < 0,
            "opposite branches at σ = {k}/4"
        );
        let gap = a.add(&b); // |a| − |b|, given the signs
        assert!(
            rat_to_f64(&gap).abs() < 1e-7,
            "the same circle at σ = {k}/4: {:.12} vs {:.12}",
            rat_to_f64(&a),
            rat_to_f64(&b)
        );
    }
}

/// **The normal cut's certificate is limited by the rail fit, not by the enclosure.**
///
/// Refining the split eightfold buys nothing measurable: what is left in ε is the degree-4 rail's
/// own departure from the surd it approximates, and no amount of σ-subdivision touches that. This
/// is the property a closed-form distance has and a first-order ball bound does not — under the
/// general quadric arm the same cut read `3.3e1` at this split, seven orders looser and shrinking
/// only as the boxes did, which is what made a device-scale annulus unaffordable.
///
/// Stated as a ratio rather than a value so it measures the *shape* of the bound: if the enclosure
/// ever becomes the binding term again, refining will start to pay and this fails.
#[test]
fn the_normal_cut_certificate_is_fit_limited_not_enclosure_limited() {
    let wall = normal_cone_wall(&q(5, 2));
    let (_, coarse) = rail(&wall, 160);
    let (_, fine) = rail(&wall, 1_280);
    assert!(
        coarse.cmp(&q(1, 1_000_000)) == core::cmp::Ordering::Less,
        "the closed-form distance certifies at the cylinder's own split: {:.3e}",
        rat_to_f64(&coarse)
    );
    assert!(
        fine.cmp(&coarse.div(&qi(2))) == core::cmp::Ordering::Greater,
        "an 8× finer split tightens by less than 2×: {:.3e} then {:.3e}",
        rat_to_f64(&coarse),
        rat_to_f64(&fine)
    );
}
