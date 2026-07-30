#![no_std]
#![forbid(unsafe_code)]
//! `lattice` — exact-arithmetic substrate for the Kirigami kernel.
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
// The unit-test harness (and the dev-only num differential backend) need `std`;
// the shipped crate stays `#![no_std]`.
#[cfg(test)]
extern crate std;

pub mod backend;
mod bignum;
mod poly;
mod rat;
mod resultant;
mod small;
mod sturm;

// Kani bounded-model-checking harnesses for the L0 fast path (compiled only
// under `cargo kani`; see vv-guide §5/§8).
#[cfg(kani)]
mod proof;

pub use backend::Backend;
pub use bignum::{BigInt, BigRat, Bignum};
pub use poly::Poly;
pub use rat::{Int, Rat};
pub use resultant::{resultant, resultant_bivariate, verify_common_factor};
pub use small::SmallRat;
pub use sturm::{Interval, SturmChain, sign_on_interval, sign_variations};
