//! The REPARAM verb (spec §7): regenerate a stalled parametrization as a canonical
//! regular record.
//!
//! When [`EDGE-REG`](crate::stall) finds a parametrization stall, the fix is not a
//! predicate but a *transform*: replace the stalled record with a regular one over a
//! canonical parameter, **superseding** it (never co-resident) and keeping a provenance
//! link back to the removed stall. [`reparam`] is that pure `old → new` transform; it
//! does not re-certify — the regenerated [`Reparam::regular`] chart is what EDGE-REG
//! re-runs against from scratch.
//!
//! The deep field-transport that *derives* the regular chart from the substitution
//! (`s = ε·χ(σ)`, spec §3.2.1) is a later milestone; here REPARAM packages a
//! caller-supplied regular chart with its supersession provenance.
//!
//! # Example
//!
//! ```
//! use geom::chart::Chart;
//! use geom::reparam::reparam;
//! use geom::stall::Stall;
//! use lattice::{Bignum, Poly, Rat, RatFunc};
//!
//! let poly = |cs: &[i128]| Poly::<Bignum>::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect());
//! let regular = Chart::new([poly(&[9]), poly(&[4]), poly(&[0, 4]), poly(&[0, 9])], RatFunc::zero());
//! let stall = Stall { sigma_star: Rat::from_i128(0), order: 1, epsilon: 1 };
//!
//! let record = reparam(stall, regular);
//! // The regenerated record is regular at σ* = 0 (|n′|² ≠ 0 there).
//! assert!(record.regular.normal_deriv_sq().eval(&Rat::from_i128(0)).unwrap().sign() != 0);
//! ```

use crate::chart::Chart;
use crate::stall::Stall;
use lattice::{Backend, Bignum};

/// A reparametrization record (spec §7): a regular chart that supersedes a stalled one,
/// with provenance back to the removed stall.
pub struct Reparam<B: Backend = Bignum> {
    /// The stall that was removed — the superseded record's defect (kept as provenance).
    pub superseded: Stall<B>,
    /// The regenerated regular chart (no stall at `σ*`).
    pub regular: Chart<B>,
}

/// Regenerate a stalled record as a regular one (spec §7): supersede `stall` with
/// `regular`, provenance-linked. A pure transform — it packages the supersession, it does
/// not re-certify (EDGE-REG re-runs on [`Reparam::regular`]).
pub fn reparam<B: Backend>(stall: Stall<B>, regular: Chart<B>) -> Reparam<B> {
    Reparam {
        superseded: stall,
        regular,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::{Poly, Rat, RatFunc};

    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Rat::from_i128(c)).collect())
    }

    #[test]
    fn reparam_supersedes_with_a_regular_record() {
        // The regular cone q = (9, 4, 4σ, 9σ) has no stall — |n′|² ≠ 0 everywhere.
        let regular = Chart::new(
            [poly(&[9]), poly(&[4]), poly(&[0, 4]), poly(&[0, 9])],
            RatFunc::zero(),
        );
        let stall = Stall {
            sigma_star: Rat::from_i128(0),
            order: 1,
            epsilon: -1,
        };
        let record = reparam(stall, regular);
        assert_eq!(record.superseded.order, 1);
        let n1_sq_0 = record
            .regular
            .normal_deriv_sq()
            .eval(&Rat::from_i128(0))
            .unwrap();
        assert!(
            n1_sq_0.sign() != 0,
            "the regenerated record is regular at σ*"
        );
    }
}
