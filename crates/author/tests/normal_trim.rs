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
//! What these pin is the thing that made it not work: the resolver labels a wall end by which side
//! of the cutter's **shadow** it is, while the oracle picks a **root** of the µ̂-quadratic, and the
//! two only agree when that quadratic opens upward. See `mu_form_opens_up`.

use arrange2d::profile::Profile;
use author::part::{Cutter, Part, SupportFn};
use certify_core::Verdict;
use develop::cone::DevConfig;
use develop::extrude::{Apex, Frame};
use export::approx::rat_to_f64;
use export::trim::RailFit;
use fixtures::devices::cone_wrap;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d)
}
fn qi(n: i128) -> Q {
    Q::from_i128(n)
}

/// The normal-cut cone through the neutral surface's circle of radius `r`.
fn normal_cone(r: &Q) -> Cutter<Bignum> {
    let z_r = r.mul(&q(-72, 65));
    let z_apex = z_r.sub(&r.mul(&q(65, 72)));
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

/// **A normal cut and a vertical cut at the same radius certify to the same ε.**
///
/// They meet the sheet in the *same circle* — the base cone and the cutter cone are coaxial
/// surfaces of revolution — so this is one geometric fact reached through two unrelated surface
/// representations: a `CutSurface::Cylinder` with a symbolic residual, and a `CutSurface::Quadric`
/// bounded by a first-order ball. Agreement to the digit is the corroboration.
///
/// The quadric route pays for it in `subdiv`: its arm encloses the traced point in a box instead
/// of cancelling the surface equation against the chart fields, so it needs a finer split for the
/// same bound — 64× here. That is a conditioning cost, not a weaker claim.
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
    let cone = eps(gore(normal_cone(&q(5, 2)), 10_240));
    assert!(
        (cyl - cone).abs() < 1e-12,
        "the same circle, two surface representations: {cyl:.6e} vs {cone:.6e}"
    );
}

/// **The quadric arm needs the finer split, and says so rather than certifying loosely.**
///
/// Non-vacuous in both directions: at the cylinder's own `subdiv` the cone is `Unresolved` — the
/// honest verdict — and it converges monotonically as the split refines.
#[test]
fn the_quadric_bound_is_unresolved_until_the_split_is_fine_enough() {
    let run = |subdiv: usize| gore(normal_cone(&q(5, 2)), subdiv).develop();
    assert!(
        matches!(run(160), Verdict::Unresolved(_)),
        "at the cylinder's subdiv the box bound is too loose to certify"
    );
    let (Verdict::Verified(coarse), Verdict::Verified(fine)) = (run(1_280), run(10_240)) else {
        panic!("both refined splits certify");
    };
    assert!(
        fine.eps().cmp(coarse.eps()) == core::cmp::Ordering::Less,
        "refining the split tightens the bound: {:.3e} then {:.3e}",
        rat_to_f64(coarse.eps()),
        rat_to_f64(fine.eps())
    );
}
