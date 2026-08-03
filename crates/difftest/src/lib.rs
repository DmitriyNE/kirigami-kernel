// `difftest` — differential oracles (shell tier, non-certified).
//
// NOT `#![forbid(unsafe_code)]`: this crate is the one place unsafe is
// permitted, and only for the C++ FFI to CGAL / OpenCascade. It never appears
// in a certified path — an oracle is compared against stored answers
// (oracle ∧ audit, never oracle-instead-of-audit), and disagreements are
// triaged as findings against either side. The FFI shim is a tiny C++ layer
// (JSON in/out).
#![deny(unsafe_op_in_unsafe_fn)]

//! CGAL `Arrangement_2` (circular kernel) and OpenCascade shape-checker oracles.

/// The CGAL FFI oracle (feature `cgal`; needs system CGAL + gmp/mpfr, i.e.
/// `nix develop`). Exposes the `Arrangement_2` and boolean oracle entry points.
#[cfg(feature = "cgal")]
pub mod cgal;

/// The CGAL `Arrangement_2` differential harness — our `arrange2d` vs CGAL, exact
/// and up to the quotient. Test-only, feature `cgal`.
#[cfg(all(test, feature = "cgal"))]
mod differential;
