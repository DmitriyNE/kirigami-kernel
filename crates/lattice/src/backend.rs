//! The exact-arithmetic backend trait — the `no_std` + `alloc` boundary.
//!
//! Per `docs/environment-and-crate-layout.md §1`, the concrete bignum backend
//! is chosen at M0 by benchmark (the yardstick: Sturm on a degree-12 polynomial
//! over 256-bit rationals) with a hard `no_std + alloc` gate. Every consumer
//! goes through this trait, so the winner is swappable without touching callers
//! and the `no_std` constraint is a property of this API rather than the crate
//! that happens to win.
//!
//! M0 defines here: the integer/rational associated types, exact
//! `cmp`/`sign`/`gcd`, interval-plus-separation comparison, cleared-forms
//! helpers, and the polynomial / Sturm / resultant operations built on them.

/// Exact rational/integer arithmetic backend. Associated types and operations
/// are added at M0; the fast-path ≡ slow-path bridge is Kani-verified there.
pub trait Backend {
    // type Int; type Rat; fn sign(..) -> ..; fn gcd(..); ...  // M0
}
