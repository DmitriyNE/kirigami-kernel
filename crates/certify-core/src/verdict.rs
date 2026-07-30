//! Three-valued verdict algebra (spec invariant 2; `vv-guide §2`).
//!
//! Never a bare `bool` for a geometric decision. A checker that cannot run
//! returns `Unresolved`, never `Verified` — soundness depends on it.

/// `Verified(Evidence) | Refuted(Witness) | Unresolved(Margin)`.
///
/// Generic over the per-certificate evidence / witness / margin payloads:
/// evidence carries the stored proof object (margins, Sturm sequences,
/// isolating intervals, stamps), the witness the refuting instance, the margin
/// the honest three-valued middle with its refinement handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict<E, W, M> {
    Verified(E),
    Refuted(W),
    Unresolved(M),
}
