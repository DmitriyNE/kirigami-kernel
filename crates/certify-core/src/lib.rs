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
//! `certify-core` — the verified checker surface.
//!
//! Pure, total, panic-free (spec invariant 4; `vv-guide §1`). This is the
//! single hax/Aeneas/Lean extraction target and the TCB boundary: it holds the
//! checkers and their algebra, nothing else. Every clever, imperative searcher
//! lives in a shell crate and flows its `(claim, certificate)` output through a
//! checker here — the searcher is never trusted.
//!
//! Checkers are organized by domain (extracted up from the shell crates per
//! `docs/environment-and-crate-layout.md §1`); the former `certify1d` crate is
//! absorbed here as the [`certify1d`] module. Modules are kept in alphabetical
//! order (rustfmt reorders them anyway); the milestone that fills each in is
//! noted in that module's own docs.

extern crate alloc;

pub mod margin;
pub mod verdict;

pub mod arrange;
pub mod cap_in;
pub mod certify1d;
pub mod free_boundary;
pub mod gate;
pub mod miter;
pub mod sew;
pub mod shell;
pub mod wedge;

// Kani bounded-model-checking harnesses for the pure checkers (compiled only under
// `cargo kani`; see `vv-guide §5/§8`) — the first Kani surface outside `lattice`.
#[cfg(kani)]
mod proof;

pub use margin::MarginSq;
pub use verdict::Verdict;
