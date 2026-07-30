#![forbid(unsafe_code)]
//! `arrange2d` — the exact D24 arrangement + boolean kernel (shell tier; M3).
//!
//! The beast: highest risk, highest reuse. Canonical decomposition (x-monotone
//! split at exact extremal points; axis-aligned tag chart — pending-v0.25), the
//! stratified event spine (most-degenerate-first; membership-before-
//! classification), the stage-2 1D coincidence lattice on shared carriers, the
//! DCEL + eight-step boolean (⊕/∧/∨, separating-edge law, faces = π₀). Every
//! output flows through the `certify_core::arrange` checkers; the CGAL oracle in
//! `difftest` runs alongside from the start.
