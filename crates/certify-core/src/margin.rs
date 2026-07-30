//! The squared-margin convention (spec invariant 7; §8.2).
//!
//! Separation margins on √-carrying quantities are declared in squared form,
//! because clearing `|x| ≥ m` to a polynomial gives `x² ≥ m²`. Comparing a
//! value against an unsquared margin it was cleared against is the bug this
//! newtype exists to make unrepresentable. Linear-scale margins, where needed,
//! are derived, T-stamped, and reporting-only.

/// A margin stored in squared form. `T` is the `lattice` rational/integer type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarginSq<T>(pub T);
