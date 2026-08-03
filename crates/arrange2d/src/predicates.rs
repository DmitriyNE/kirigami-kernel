//! Exact line predicates. `PARALLEL` := `a_A·b_B − a_B·b_A = 0`
//! (the direction cross, one ring op); `COINCIDENT` := all three 2×2 minors of
//! the stacked `(a, b, c)` rows vanish (kept in three-minor form — it cannot be
//! half-read; the `(a, b)` normal minor alone is WRONG). Plus circle
//! carrier-coincidence (equal center ∧ equal `r²`). Corpus: `cx_parallel_distinct_lines`.

use geom::content::{Circle, Line};
use lattice::{Backend, Rat};

/// The `a_A·b_B − a_B·b_A` minor of the stacked normals — the shared kernel of
/// both `PARALLEL` (this alone) and `COINCIDENT` (this plus two more). Exact ℚ;
/// also the Cramer determinant of the line∩line solve, so it lives here.
pub(crate) fn minor_ab<B: Backend>(la: &Line<B>, lb: &Line<B>) -> Rat<B> {
    la.a.mul(&lb.b).sub(&la.b.mul(&lb.a))
}

/// `PARALLEL`: the two directions are collinear ⇔ the normal cross
/// `a_A·b_B − a_B·b_A` vanishes. Coincident lines are also parallel.
pub fn parallel<B: Backend>(la: &Line<B>, lb: &Line<B>) -> bool {
    minor_ab(la, lb).is_zero()
}

/// `COINCIDENT`: the stacked rows `(a, b, c)` collapse ⇔ **all three** 2×2 minors
/// of the 2×3 matrix vanish. Kept in three-minor form on purpose — the
/// `(a, b)` minor alone (i.e. `PARALLEL`) is necessary but *not* sufficient; a
/// coincident pair also needs the `(a, c)` and `(b, c)` minors to vanish, else
/// two parallel-but-offset lines would read as the same carrier. Implies
/// [`parallel`].
pub fn coincident<B: Backend>(la: &Line<B>, lb: &Line<B>) -> bool {
    minor_ab(la, lb).is_zero()
        && la.a.mul(&lb.c).sub(&la.c.mul(&lb.a)).is_zero() // (a, c) minor
        && la.b.mul(&lb.c).sub(&la.c.mul(&lb.b)).is_zero() // (b, c) minor
}

/// Two circles share a carrier ⇔ equal center **and** equal squared radius — all
/// three compared exactly over ℚ (spec §2.2: never the irrational `r`).
pub fn circles_coincident<B: Backend>(ca: &Circle<B>, cb: &Circle<B>) -> bool {
    ca.cx == cb.cx && ca.cy == cb.cy && ca.r2 == cb.r2
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Bignum;

    type Q = Rat<Bignum>;

    fn line(a: i128, b: i128, c: i128) -> Line<Bignum> {
        Line {
            a: Q::from_i128(a),
            b: Q::from_i128(b),
            c: Q::from_i128(c),
        }
    }
    fn circle(cx: i128, cy: i128, r2: i128) -> Circle<Bignum> {
        Circle {
            cx: Q::from_i128(cx),
            cy: Q::from_i128(cy),
            r2: Q::from_i128(r2),
        }
    }

    /// Corpus `cx_parallel_distinct_lines`: `y = 0` and `y = 1` are parallel but
    /// not coincident (only the `(a, b)` minor vanishes; `(b, c)` does not).
    #[test]
    fn cx_parallel_distinct_lines() {
        let l0 = line(0, 1, 0); // y = 0
        let l1 = line(0, 1, -1); // y = 1
        assert!(parallel(&l0, &l1));
        assert!(!coincident(&l0, &l1));
    }

    #[test]
    fn transverse_lines_not_parallel() {
        let x_axis = line(0, 1, 0);
        let y_axis = line(1, 0, 0);
        assert!(!parallel(&x_axis, &y_axis));
        assert!(!coincident(&x_axis, &y_axis));
    }

    /// A scaled copy of a line is the same carrier: `2x + 4y + 6 = 0` ≡
    /// `x + 2y + 3 = 0`. All three minors vanish.
    #[test]
    fn coincident_lines_scaled_copy() {
        let l = line(1, 2, 3);
        let scaled = line(2, 4, 6);
        assert!(parallel(&l, &scaled));
        assert!(coincident(&l, &scaled));
    }

    /// Same normal, different offset ⇒ parallel, NOT coincident — the case the
    /// two extra minors exist to reject.
    #[test]
    fn parallel_offset_is_not_coincident() {
        let l = line(1, 2, 3);
        let offset = line(1, 2, 5);
        assert!(parallel(&l, &offset));
        assert!(!coincident(&l, &offset));
    }

    #[test]
    fn circle_carrier_coincidence() {
        let c = circle(1, 2, 9);
        assert!(circles_coincident(&c, &circle(1, 2, 9)));
        assert!(!circles_coincident(&c, &circle(1, 2, 4))); // same center, r²≠
        assert!(!circles_coincident(&c, &circle(0, 2, 9))); // center≠
    }
}
