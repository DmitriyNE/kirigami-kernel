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
//!
//! **M3a** builds the front half — canonical decomposition + the event spine
//! (spec §6 steps 1–4); the DCEL / eight-step boolean / CAP-OUT are later slices.
//! `arrange2d` is an untrusted *searcher*; soundness lives in the M3e
//! `certify_core::arrange` checkers. The modules, filled across the M3a phases:
//!   * [`predicates`] — PARALLEL / COINCIDENT + circle carrier-coincidence.
//!   * [`carrier`]    — carrier ∩ carrier → degree-≤2 `Surd` points.
//!   * [`decompose`]  — canonical x-monotone decomposition (pending-v0.25).
//!   * [`membership`] — per-edge interval membership, before classification.
//!   * [`classify`]   — transverse/tangent + sidedness bits.
//!   * [`event`]      — the `Event` / `EventSet` the spine emits.
//!   * [`spine`]      — the steps-1–4 driver, most-degenerate-first.
//!   * [`witness`]    — the replayable `(claim, certificate)` for the M3e checker.

pub mod azimuth;
pub mod boolean;
pub mod carrier;
pub mod classify;
pub mod coincide;
pub mod dcel;
pub mod decompose;
pub mod event;
pub mod membership;
pub mod predicates;
pub mod spine;
pub mod tangent;
pub mod witness;

/// Shared test-only V&V support (generators + independent oracles), Phase 5.
#[cfg(test)]
mod testgen;
