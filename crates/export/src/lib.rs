#![forbid(unsafe_code)]
//! `export` — STEP + mesh + marks (shell tier; M8).
//!
//! Planar trims of ruled faces, rational patches with IDEALIZED flags, the mesh
//! size field `min(s_max, 1/κ₁)`, dimension/mark layers, and the GRID-closure
//! rounding ledger. Every export carries the two-field stamp
//! `{semantics, status}`. Floats live only in this crate behind the
//! `diagnostics` feature (plots and viewers) — never in a value that carries a
//! certificate.

/// Exact→`f64` approximation for diagnostics rendering — the single, quarantined
/// bridge from the certified-exact number types to display floats.
#[cfg(feature = "diagnostics")]
pub mod approx;

/// 2D SVG rendering of certified boolean regions (extractor + `<svg>` + gallery page).
/// Builds on [`approx`] — floats touch the display only, never a predicate.
#[cfg(feature = "diagnostics")]
pub mod svg;
