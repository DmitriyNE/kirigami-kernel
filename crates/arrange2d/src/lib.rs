#![forbid(unsafe_code)]
//! Exact 2D arrangements and boolean operations over lines and circular arcs.
//!
//! `arrange2d` computes, with **no floating point**, the arrangement of a set of
//! straight segments and circular arcs (the "D24" content class) and boolean
//! combinations — union, intersection, symmetric difference — of two regions bounded
//! by them. All coordinates are exact algebraic numbers of the form `a + b√d`
//! ([`lattice::Surd`]); intersection points, tangencies, and boundaries are computed
//! and compared exactly, so results are deterministic and free of the robustness
//! failures that plague floating-point geometry.
//!
//! # What you can do
//!
//! - **Boolean of two regions** — the main entry point. Assign each input curve to
//!   operand `A` or `B`, pick an operation, and get the result region as faces (each an
//!   outer boundary loop plus counter-oriented holes):
//!   - [`boolean::ledge_dom`] — fast, emits unconditionally.
//!   - [`boolean::ledge_dom_certified`] — the same result, but every output is checked
//!     by the proven checkers below and a defect is reported as a
//!     [`boolean::CapOutFault`] rather than a silently-wrong region.
//! - **The arrangement itself** — [`spine::arrange_events`] returns the intersection
//!   vertices and coincidence structure; [`dcel::Dcel::build`] builds the half-edge
//!   arrangement the boolean runs on.
//!
//! # Example
//!
//! Union two overlapping disks. Each input curve is decomposed into arrangement
//! [`Edge`](geom::content::Edge)s carrying a [`CurveId`](geom::content::CurveId);
//! `operand_of` maps each id to operand `A` or `B`.
//!
//! ```
//! use arrange2d::boolean::{ledge_dom, BoolOp, OperandId};
//! use arrange2d::decompose::decompose;
//! use geom::content::{Circle, Curve, CurveId, Orient};
//! use lattice::{Bignum, Rat};
//!
//! // A disk as decomposed edges: centre (cx, cy), squared radius r2, tagged `src`.
//! let disk = |cx, cy, r2, src| {
//!     decompose(&Curve::Circle {
//!         circle: Circle {
//!             cx: Rat::<Bignum>::from_i128(cx),
//!             cy: Rat::from_i128(cy),
//!             r2: Rat::from_i128(r2),
//!         },
//!         orient: Orient::Ccw,
//!         source: CurveId(src),
//!     })
//! };
//!
//! let mut edges = disk(0, 0, 25, 0); // operand A: centre (0,0), r² = 25
//! edges.extend(disk(8, 0, 25, 1));   // operand B: centre (8,0), r² = 25
//!
//! let region = ledge_dom(
//!     &edges,
//!     &|c: CurveId| if c.0 == 0 { OperandId::A } else { OperandId::B },
//!     BoolOp::Or,
//! );
//!
//! // A ∪ B of two overlapping disks is a single face with no holes.
//! assert_eq!(region.faces.len(), 1);
//! assert!(region.faces[0].holes.is_empty());
//! ```
//!
//! # Trust model
//!
//! This crate is an untrusted **constructor**: it may use any algorithm to produce a
//! candidate answer. Correctness is established separately by the small, pure, formally
//! verified **checkers** in [`certify_core::arrange`] (the ℤ₂² cocycle, CAP-OUT-LINK,
//! and Link≅geom checks, all Kani-proven), which consume a flat certificate this crate
//! emits. [`boolean::ledge_dom_certified`] runs those checkers over the emitted region.
//! An independent CGAL oracle (in the `difftest` crate) cross-checks the exact geometry.
//!
//! # Modules
//!
//! Region layer: [`boolean`] (the boolean engine + certificate), [`dcel`] (the half-edge
//! arrangement), [`locate`] (exact point-in-region ray-casting), [`tangent`] (the vertex
//! rotation order). Arrangement layer: [`spine`] (the driver), [`carrier`] (curve∩curve
//! solving), [`decompose`] (x-monotone splitting), [`predicates`]/[`classify`]/
//! [`membership`] (the geometric predicates), [`coincide`]/[`azimuth`] (shared-carrier
//! overlaps), [`event`]/[`witness`] (the emitted event records).

pub mod azimuth;
pub mod boolean;
pub mod carrier;
pub mod classify;
pub mod coincide;
pub mod dcel;
pub mod decompose;
pub mod event;
pub mod locate;
pub mod membership;
pub mod predicates;
pub mod profile;
pub mod spine;
pub mod tangent;
pub mod witness;

/// Shared test-only support: random-input generators and independent oracles.
#[cfg(test)]
mod testgen;
