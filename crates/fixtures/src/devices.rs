//! The normative device instances (spec §13).
//!
//! Currently the **cone** ([`cone`]): a rational right circular cone with apex at the
//! origin (`CONE(0)`) and half-angle ≈ 42° (`n·ẑ = 65/97 ≈ sin 42.07°`). The kernel is
//! exact over ℚ, so the spec's β = 42° is realized by the nearest convenient rational
//! cone; the device is a golden/validation instance, its geometry checked to tolerance.
//!
//! The petal conical flank (the general-case adversary) is not yet pinned by spec §13
//! and lands with milestone C.

use geom::chart::Chart;
use lattice::{Bignum, Poly, Rat, RatFunc};

/// The device cone (spec §13): a rational right circular cone, apex at the origin, axis
/// `ẑ`, half-angle ≈ 42°.
///
/// Built from `q(σ) = (9, 4, 4σ, 9σ)` with `h ≡ 0` (rulings through the origin). Its
/// normal satisfies the exact cone invariant `n·ẑ ≡ 65/97`.
pub fn cone() -> Chart<Bignum> {
    let poly = |cs: &[i128]| Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
    let q = [poly(&[9]), poly(&[4]), poly(&[0, 4]), poly(&[0, 9])];
    Chart::new(q, RatFunc::zero())
}

#[cfg(test)]
mod tests {
    use super::*;
    use geom::tags::{Tag, classify};

    #[test]
    fn cone_is_a_cone_through_the_origin() {
        let apex = [Rat::from_i128(0), Rat::from_i128(0), Rat::from_i128(0)];
        assert_eq!(classify(&cone()), Some(Tag::Cone { apex }));
    }

    #[test]
    fn cone_axis_angle_exact_and_near_42_degrees() {
        let c = cone();
        // Exact cone invariant: n·ẑ ≡ 65/97 (constant along and across rulings).
        let nz = c.normal().comp(2);
        let want = RatFunc::from_poly(Poly::<Bignum>::constant(Rat::new(65, 97)));
        assert_eq!(nz, want);

        // Validation to tolerance (exact rationals, no floats): 65/97 vs sin 42° ≈ 669/1000.
        let diff = Rat::<Bignum>::new(65, 97).sub(&Rat::new(669, 1000));
        let tol = Rat::new(1, 100);
        assert!(
            diff < tol && diff > tol.neg(),
            "half-angle within ~1° of 42°"
        );
    }
}
