// `forbid(unsafe_code)` by default; the `step` feature pulls in the `cxx` OCCT
// bridge (generated `unsafe extern "C++"`), so under `step` this relaxes to
// `deny` with a single scoped `#[allow(unsafe_code)]` on the quarantined
// `step` module — the only unsafe in the crate.
#![cfg_attr(not(feature = "step"), forbid(unsafe_code))]
#![cfg_attr(feature = "step", deny(unsafe_code))]
//! `export` — STEP + mesh + marks (shell tier; M8).
//!
//! Planar trims of ruled faces, rational patches with IDEALIZED flags, the mesh
//! size field `min(s_max, 1/κ₁)`, dimension/mark layers, and the GRID-closure
//! rounding ledger. Every export carries the two-field stamp
//! `{semantics, status}`. Floats live only in this crate behind the
//! `diagnostics` feature (plots and viewers) — never in a value that carries a
//! certificate.

/// The neutral exact shell record (triangulated boundary of a certified one-joint
/// closure) — always compiled and float-free; the geometry the STEP writer consumes.
pub mod shell;

/// Exact→`f64` approximation for diagnostics rendering and the STEP writer — the
/// single, quarantined bridge from the certified-exact number types to display floats.
#[cfg(any(feature = "diagnostics", feature = "step"))]
pub mod approx;

/// 2D SVG rendering of certified boolean regions (extractor + `<svg>` + gallery page).
/// Builds on [`approx`] — floats touch the display only, never a predicate.
#[cfg(feature = "diagnostics")]
pub mod svg;

/// 3D rendering of the certified cone strip (surface sampler + Three.js viewer page).
/// Builds on [`approx`] — floats touch the display only, never a predicate.
#[cfg(feature = "diagnostics")]
pub mod mesh3d;

/// Real `.step` export via a `cxx` C++ FFI shim to OpenCASCADE's `STEPControl_Writer`.
/// Behind the off-by-default `step` feature (needs system OCCT; build under `nix develop`).
#[cfg(feature = "step")]
#[allow(unsafe_code)] // the cxx bridge — the sole quarantined unsafe surface
pub mod step;
