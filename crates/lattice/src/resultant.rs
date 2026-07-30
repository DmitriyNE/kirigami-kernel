//! Resultants over ℚ — variable elimination / common-root detection
//! (`docs/agent-glossary.md`; spec §5.3 MITER-FIT `R(σ_A,σ_B)=0`, §8.5 EDGE-EMB).
//! Pure, `no_std`, total.
//!
//! Two computations: the **numeric** resultant of `f, g ∈ ℚ[x]` (a scalar) via
//! the Euclidean recurrence — robust, no matrix pivoting; and the **bivariate**
//! resultant (eliminate `x` between two polynomials whose coefficients are
//! `y`-polynomials) via the Sylvester matrix and fraction-free Bareiss.
//!
//! **Verification (vv-guide §0, `proofs/ledger.md`):** the resultant⇔common-root
//! theorem is cited; the instance certificate is a **divisibility check** —
//! [`verify_common_factor`] — which is exactly the spec's "resultant-conditioned
//! A-identity (divisibility check)" (§5.3). The resultant *value* is
//! cross-checked differentially against the independent `Poly::gcd`. Resultants
//! are out of Kani scope (vv-guide §5).

use crate::backend::Backend;
use crate::poly::Poly;
use crate::rat::Rat;
use alloc::vec;
use alloc::vec::Vec;

fn rat_pow<B: Backend>(base: &Rat<B>, exp: usize) -> Rat<B> {
    let mut r = Rat::from_i128(1);
    for _ in 0..exp {
        r = r.mul(base);
    }
    r
}

/// The resultant of `f, g ∈ ℚ[x]`. `Res(f, g) == 0` iff `f, g` share a
/// positive-degree factor (a common root). Computed by the Euclidean recurrence
/// `Res(f,g) = (−1)^{mn}·lead(g)^{m−s}·Res(g, f mod g)`.
pub fn resultant<B: Backend>(f: &Poly<B>, g: &Poly<B>) -> Rat<B> {
    if f.is_zero() || g.is_zero() {
        return Rat::from_i128(0);
    }
    let mut f = f.clone();
    let mut g = g.clone();
    let mut acc = Rat::from_i128(1);
    loop {
        let m = f.degree().unwrap();
        let n = g.degree().unwrap();
        if m < n {
            core::mem::swap(&mut f, &mut g);
            if (m * n) % 2 == 1 {
                acc = acc.neg();
            }
            continue;
        }
        if n == 0 {
            // g is a nonzero constant b: Res(f, b) = b^{deg f}.
            return acc.mul(&rat_pow(g.leading().unwrap(), m));
        }
        let r = f.rem(&g);
        if r.is_zero() {
            return Rat::from_i128(0); // g | f, deg g ≥ 1 ⇒ common factor
        }
        let s = r.degree().unwrap();
        if (m * n) % 2 == 1 {
            acc = acc.neg();
        }
        acc = acc.mul(&rat_pow(&g.leading().unwrap().clone(), m - s));
        f = g;
        g = r;
    }
}

/// Runtime-checked hypothesis (spec §5.3 divisibility check): `h` (degree ≥ 1)
/// divides both `f` and `g` — a certificate that they share a common root.
pub fn verify_common_factor<B: Backend>(f: &Poly<B>, g: &Poly<B>, h: &Poly<B>) -> bool {
    h.degree().is_some_and(|d| d >= 1) && f.rem(h).is_zero() && g.rem(h).is_zero()
}

// ---- bivariate: Sylvester matrix + fraction-free Bareiss over a polynomial ----

/// An integral domain with exact division (divisibility guaranteed by the
/// Bareiss recurrence). Implemented for `Poly<B>` (the bivariate coefficient ring).
pub trait Domain: Clone {
    fn zero() -> Self;
    fn sub(&self, o: &Self) -> Self;
    fn mul(&self, o: &Self) -> Self;
    /// Exact quotient; `o | self` by construction (Bareiss).
    fn div_exact(&self, o: &Self) -> Self;
    fn is_zero(&self) -> bool;
}

impl<B: Backend> Domain for Poly<B> {
    fn zero() -> Self {
        Poly::zero()
    }
    fn sub(&self, o: &Self) -> Self {
        Poly::sub(self, o)
    }
    fn mul(&self, o: &Self) -> Self {
        Poly::mul(self, o)
    }
    fn div_exact(&self, o: &Self) -> Self {
        let (q, r) = self.divrem(o);
        debug_assert!(r.is_zero(), "Bareiss div_exact: non-exact division");
        q
    }
    fn is_zero(&self) -> bool {
        Poly::is_zero(self)
    }
}

/// Fraction-free Bareiss determinant with row-swap pivoting on zero pivots.
pub fn bareiss_det<D: Domain>(mut m: Vec<Vec<D>>) -> D {
    let n = m.len();
    if n == 0 {
        return D::zero(); // convention: empty handled by callers
    }
    let mut prev = D::zero(); // the previous pivot; the first step divides by "1"
    let mut first = true;
    let mut negate = false;
    for k in 0..n - 1 {
        if m[k][k].is_zero() {
            match (k + 1..n).find(|&i| !m[i][k].is_zero()) {
                Some(i) => {
                    m.swap(k, i);
                    negate = !negate;
                }
                None => return D::zero(), // a zero column ⇒ singular
            }
        }
        for i in k + 1..n {
            for j in k + 1..n {
                let num = m[i][j].mul(&m[k][k]).sub(&m[i][k].mul(&m[k][j]));
                m[i][j] = if first { num } else { num.div_exact(&prev) };
            }
        }
        prev = m[k][k].clone();
        first = false;
    }
    let det = m[n - 1][n - 1].clone();
    if negate { D::zero().sub(&det) } else { det }
}

fn sylvester<D: Domain>(fc_hilo: &[D], gc_hilo: &[D]) -> Vec<Vec<D>> {
    let m = fc_hilo.len() - 1;
    let n = gc_hilo.len() - 1;
    let size = m + n;
    let mut mat = vec![vec![D::zero(); size]; size];
    for (r, row) in mat.iter_mut().enumerate().take(n) {
        for (j, c) in fc_hilo.iter().enumerate() {
            row[r + j] = c.clone();
        }
    }
    for r in 0..m {
        for (j, c) in gc_hilo.iter().enumerate() {
            mat[n + r][r + j] = c.clone();
        }
    }
    mat
}

/// Eliminate `x` between two polynomials whose `x`-coefficients are `y`-polynomials
/// (`f`, `g` given low-`x`-degree first, each leading `Poly` nonzero, both of
/// `x`-degree ≥ 1). Returns the eliminant in `y`. `Res_x = 0` (as a `y`-poly)
/// where the two share a common `x`-root.
pub fn resultant_bivariate<B: Backend>(f: &[Poly<B>], g: &[Poly<B>]) -> Poly<B> {
    debug_assert!(
        f.len() >= 2 && g.len() >= 2,
        "resultant_bivariate: x-degree ≥ 1"
    );
    let fc: Vec<Poly<B>> = f.iter().rev().cloned().collect();
    let gc: Vec<Poly<B>> = g.iter().rev().cloned().collect();
    bareiss_det(sylvester(&fc, &gc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bignum::Bignum;

    type P = Poly<Bignum>;
    type Q = Rat<Bignum>;
    fn p(cs: &[i128]) -> P {
        P::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }
    fn lin(r: i128) -> P {
        p(&[-r, 1]) // x − r
    }

    #[test]
    fn numeric_resultant() {
        // (x−2)(x−3) and (x−1)(x−3) share the root 3 ⇒ Res = 0.
        assert!(resultant(&lin(2).mul(&lin(3)), &lin(1).mul(&lin(3))).is_zero());
        // x²−2 and x²−3 are coprime ⇒ Res = (a−b)² = 1 ≠ 0.
        let r = resultant(&p(&[-2, 0, 1]), &p(&[-3, 0, 1]));
        assert!(!r.is_zero());
        assert_eq!(r, Q::from_i128(1));
        // shared factor certified by divisibility (spec §5.3)
        let f = lin(2).mul(&lin(3));
        let g = lin(1).mul(&lin(3));
        assert!(verify_common_factor(&f, &g, &lin(3)));
        assert!(!verify_common_factor(&f, &g, &lin(5)));
    }

    #[test]
    fn resultant_zero_iff_common_factor() {
        // Res = 0  ⇔  deg gcd ≥ 1, cross-checked against the independent Poly::gcd.
        for (fa, ga) in [
            (lin(1).mul(&lin(2)), lin(2).mul(&lin(3))), // share (x−2)
            (lin(1).mul(&lin(4)), lin(2).mul(&lin(3))), // coprime
            (p(&[1, 0, 1]), p(&[-1, 0, 1])),            // x²+1, x²−1 coprime
        ] {
            let res_zero = resultant(&fa, &ga).is_zero();
            let gcd_nontrivial = fa.gcd(&ga).degree().is_some_and(|d| d >= 1);
            assert_eq!(res_zero, gcd_nontrivial);
        }
    }

    #[test]
    fn bivariate_nodal_cubic() {
        // cx-nodal-cubic self-intersection e(t)=(t²−1, t³−t). With t1,t2 the two
        // parameters: g1 = t1 + t2, g2 = t1² + t1·t2 + t2² − 1. Eliminate t1:
        //   Res_{t1}(g1, g2) = g2(−t2) = t2² − 1  → nodes at t2 = ±1.
        // g1 as t1-coeffs (low→high): [t2, 1]
        let g1 = vec![p(&[0, 1]), P::constant(Q::from_i128(1))]; // t2, 1
        // g2 as t1-coeffs: [t2²−1, t2, 1]
        let g2 = vec![p(&[-1, 0, 1]), p(&[0, 1]), P::constant(Q::from_i128(1))];
        let elim = resultant_bivariate(&g1, &g2);
        assert_eq!(elim, p(&[-1, 0, 1])); // t2² − 1
        // its roots (±1) are the node parameters
        assert_eq!(crate::sturm::SturmChain::new(&elim).count_all(), 2);
    }
}
