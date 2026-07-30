#![no_std]
#![forbid(unsafe_code)]
//! `lattice` — exact-arithmetic substrate for the flex-substrate kernel.
//!
//! Pure tier (see `docs/environment-and-crate-layout.md §1`): everything above
//! imports only this for numbers. Two-tier lattice (spec invariant 5): an L0
//! fixed-limb fast path (Kani-verified) over a BigInt slow path (the semantic
//! reference matching Lean's `Int`/`Rat`). Never inline raw bignum ops
//! elsewhere — go through `lattice`.
//!
//! M0 fills this in: exact cmp/sign/gcd, polynomial arithmetic, Sturm sequences
//! (isolation + sign-on-interval), and bivariate resultants — all behind the
//! [`backend::Backend`] trait so the concrete bignum crate stays swappable.

extern crate alloc;

pub mod backend;
