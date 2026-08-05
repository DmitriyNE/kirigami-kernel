#![no_std]
#![forbid(unsafe_code)]
// Pure-tier panic-freedom (docs/trusted-invariants.md): the panic-capable constructs are
// forbidden in production code. Every surviving `#[allow]` must carry a `// PANIC-FREEDOM:`
// tag, checked by `cargo xtask lint`. Tests may panic freely (gated by `not(test)`).
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]
//! `lattice` — exact-arithmetic substrate for the Kirigami kernel.
//!
//! Pure tier (see `docs/environment-and-crate-layout.md §1`): everything above
//! imports only this for numbers. Two-tier lattice (spec invariant 5): an L0
//! fixed-limb fast path (Kani-verified) over a BigInt slow path (the semantic
//! reference matching Lean's `Int`/`Rat`). Never inline raw bignum ops
//! elsewhere — go through `lattice`.
//!
//! Provides: exact cmp/sign/gcd, polynomial arithmetic, Sturm sequences (root
//! isolation and sign-on-interval), and bivariate resultants — all behind the
//! [`backend::Backend`] trait so the concrete bignum crate stays swappable.

extern crate alloc;
// The unit-test harness (and the dev-only num differential backend) need `std`;
// the shipped crate stays `#![no_std]`.
#[cfg(test)]
extern crate std;

mod algebraic;
pub mod backend;
mod bignum;
mod poly;
mod rat;
mod ratfunc;
// Differential-fuzz core (op-chain dashu ≡ proven RefBackend over large operands).
// Test/fuzz-only: reached via `test` cfg (proptest) or the `fuzzing` feature (cargo-fuzz).
#[cfg(any(test, feature = "fuzzing"))]
pub mod ratfuzz;
mod refbackend;
mod resultant;
mod small;
mod sturm;

// Kani bounded-model-checking harnesses for the L0 fast path (compiled only
// under `cargo kani`; see vv-guide §5/§8).
#[cfg(kani)]
mod proof;

pub use algebraic::{Alg, AlgReal, Surd};
pub use backend::Backend;
pub use bignum::{BigInt, BigRat, Bignum};
pub use poly::Poly;
pub use rat::{Int, Rat};
pub use ratfunc::{RatFunc, Vec3Rat};
pub use refbackend::{RefBackend, RefInt, RefRat};
pub use resultant::{resultant, resultant_bivariate, verify_common_factor};
pub use small::SmallRat;
pub use sturm::{Interval, SturmChain, sign_on_interval, sign_variations};
