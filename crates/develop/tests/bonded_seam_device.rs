//! DD.4 — the **acceptance demo**: the certified BONDED seam device end-to-end. The self-lapping
//! cone-with-ramp is realized as **body gore (γ = 0) + ramp flap (γ ≠ 0) + a certified bond**
//! (§6.2 — a lap is doubled material). This test composes the whole flex-PCB spine on it:
//!
//! - the **ramp flap** develops to a certified flat pattern (DD.2, `γ ≠ 0`) and a flat point folds
//!   back onto it (DD.3, the signed-µ̂ directrix residual);
//! - the **body gore** develops and folds (the γ = 0 round-trip, DD.1/DEV.2d/2e);
//! - the seam **bond** is certified by the Stage-2 §14 conjunction `valid_bonded_seam`.
//!
//! The flat pattern is sampled at the band corners via the certified `dev.point` (DD.2) — the
//! boundary-loop `unroll` composes the *same* `point_on`, so it rides unchanged, but for a `γ ≠ 0`
//! chart its anchor subdivision re-integrates γ per sub-interval (slow — see the DD.4 perf note),
//! so the fast composed test samples corners. The two STEP solids + OCCT `audit_brep` are the
//! step-gated `export::brep_build::bonded_lap_seam_two_certified_solids_plus_a_bond` test.

use certify_core::Verdict;
use develop::bonded::{LapRail, clear, sep, shear, slab, valid_bonded_seam};
use develop::cone::{ConeDevelopment, DevConfig};
use develop::fold::fold_point;
use fixtures::devices::{cone_seam, cone_seam_ramp};
use lattice::{Bignum, Interval, Poly, Rat, RatFunc};

type Q = Rat<Bignum>;

fn ratf(n: i128, d: i128) -> RatFunc<Bignum> {
    RatFunc::from_poly(Poly::from_coeffs(vec![Rat::new(n, d)]))
}

/// A verdict tag that skips the (non-`Debug`) `Verified` payload — for panic messages.
fn why<T, E: core::fmt::Debug, M: core::fmt::Debug>(v: &Verdict<T, E, M>) -> String {
    match v {
        Verdict::Verified(_) => "Verified".into(),
        Verdict::Refuted(e) => format!("Refuted({e:?})"),
        Verdict::Unresolved(m) => format!("Unresolved({m:?})"),
    }
}

/// Develop the band's four corners `σ' ∈ {0, 1/2}, µ̂ ∈ {−2, −1}` to certified flat points and
/// assert each clears the DRC (`backward_error < clearance/2`) — the certified flat pattern.
fn develop_flat_pattern(dev: &ConeDevelopment<Bignum>, cfg: &DevConfig<Bignum>) {
    let half = Q::new(1, 2); // clearance/2 for clearance = 1
    for &(sn, sd) in &[(0i128, 1i128), (1, 2)] {
        for &m in &[-2i128, -1] {
            let box_ = dev.point(&Q::new(sn, sd), &Q::from_i128(m), cfg);
            assert!(
                box_.backward_error().cmp(&half) == core::cmp::Ordering::Less,
                "corner (σ'={sn}/{sd}, µ̂={m}) develops within the DRC (ε = {:?})",
                box_.backward_error()
            );
        }
    }
}

#[test]
fn the_bonded_seam_device_round_trips_and_bonds() {
    let cfg = DevConfig::tight();
    let clearance = Q::from_i128(1);
    let w0 = Q::from_i128(0);
    let band = Interval {
        lo: Q::from_i128(0),
        hi: Q::new(1, 2),
    };

    // ---- THE RAMP FLAP (γ ≠ 0): develop (DD.2) then fold (DD.3) ----
    let flap = cone_seam_ramp();
    let flap_dev = ConeDevelopment::new_developable(&flap, 64).expect("the flap is a developable");
    develop_flat_pattern(&flap_dev, &cfg); // DD.2 γ≠0 develop, certified per corner
    // FOLD (direction ②): a flat point on the flap folds back to its (σ', µ̂) — µ̂ < 0 exercises the
    // signed-residual `flip`.
    let (s0, m0) = (Q::new(1, 4), Q::new(-3, 2));
    let (fx, fy) = flap_dev.point(&s0, &m0, &cfg).center();
    match fold_point(&flap, &fx, &fy, &w0, &band, 40, true, &cfg, &clearance) {
        Verdict::Verified(f) => {
            assert!(f.sigma.contains(&s0), "flap fold recovers σ' = 1/4");
            assert!(f.mu.contains(&m0), "flap fold recovers µ̂ = −3/2");
        }
        other => panic!("the flap fold (γ≠0) must certify: {}", why(&other)),
    }

    // ---- THE BODY GORE (γ = 0): develop then fold ----
    let body = cone_seam();
    let body_dev = ConeDevelopment::new(&body).expect("the body is an apex cone");
    develop_flat_pattern(&body_dev, &cfg); // γ=0 develop
    let (bs, bm) = (Q::new(1, 4), Q::new(-3, 2));
    let (bx, by) = body_dev.point(&bs, &bm, &cfg).center();
    match fold_point(&body, &bx, &by, &w0, &band, 40, true, &cfg, &clearance) {
        Verdict::Verified(f) => {
            assert!(f.sigma.contains(&bs), "body fold recovers σ' = 1/4");
            assert!(f.mu.contains(&bm), "body fold recovers µ̂ = −3/2");
        }
        other => panic!("the body fold (γ=0) must certify: {}", why(&other)),
    }

    // ---- THE CERTIFIED BOND (Stage 2 §14 BONDED) ----
    let sig = Interval {
        lo: Q::new(-1, 4),
        hi: Q::new(1, 4),
    };
    let neg1 = Q::from_i128(-1);
    let bond = valid_bonded_seam(
        // SEP: plateau separation ≡ the bond gap Δ = g = 1/4 (base h = 0, plateau h = 1/4).
        sep(
            &RatFunc::<Bignum>::zero(),
            &w0,
            &ratf(1, 4),
            &w0,
            &Q::new(1, 4),
        ),
        // SLAB: the offset slab stays regular over the seam box at the µ = −1 corner.
        slab(&cone_seam_ramp(), &neg1, &w0, &sig, &Q::new(1, 1000)),
        // SHEAR: κ_g = −65/72 (−tan β), Δ₀ = 1/4 ⇒ δ = 18/65 ≈ 0.28 mm.
        shear(&ratf(-65, 72), &ratf(1, 4), &Q::new(1, 100)),
        // CLEAR: the base rail and the ramp rail keep clear over the seam box.
        clear(
            &LapRail::from_chart(&cone_seam(), &neg1, &w0),
            &LapRail::from_chart(&cone_seam_ramp(), &neg1, &w0),
            &sig,
            &Q::new(1, 8),
            2000,
        ),
    );
    assert!(
        matches!(bond, Verdict::Verified(_)),
        "the §14 BONDED conjunction certifies the seam"
    );
}
