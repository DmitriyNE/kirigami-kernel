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

use author::construct;
use author::part::{Cutter, Part, SupportFn};
use develop::cone::DevConfig;
use export::trim::RailFit;
use fixtures::devices::{cone, cone_wrap};
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
    part
}

/// The centre of the self-lapping device's seam drill, `(x, y, r²)` — the 3-D cylinder both
/// derived holes must fold back onto. Exposed so a round-trip check tests the *same* cylinder the
/// part was cut with instead of restating its numbers.
pub fn seam_drill_axis() -> (Q, Q, Q) {
    (q(-1, 2), q(27, 10), q(1, 40))
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
