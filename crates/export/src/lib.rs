#![forbid(unsafe_code)]
//! `export` — STEP + mesh + marks (shell tier; M8).
//!
//! Planar trims of ruled faces, rational patches with IDEALIZED flags, the mesh
//! size field `min(s_max, 1/κ₁)`, dimension/mark layers, and the GRID-closure
//! rounding ledger. Every export carries the two-field stamp
//! `{semantics, status}`. Floats live only in this crate behind the
//! `diagnostics` feature (plots and viewers) — never in a value that carries a
//! certificate.
