#![forbid(unsafe_code)]
//! `develop` — the flat side (shell tier; M7).
//!
//! The development map `D(σ, μ̂)` and the `γ` ODE per tag, the content layer and
//! flat booleans (reusing the `arrange2d` kernel), folds/reflection mates, the
//! seam, and calibration. Cold-layer machinery: required for the material grade
//! and fab exports, not for the closure vertical slice. Its riders sweep the
//! cold layers late — expect findings and an adversarial pass.
//!
//! **Milestone-E spike (DEV.1).** The first concrete slice is the certified
//! *cone* development: [`interval`] holds the float-free rational enclosures of
//! the elementary transcendentals (`arctan`, `π`, `cos`, `sin`, `√`), and
//! [`cone`] turns a cone [`Chart`](geom::chart::Chart) into a certified flat
//! point `D(σ, μ̂)` with a rational backward-error bound and a `DRC` verdict. See
//! `docs/spike-development-report.md` for the method choice and the GO call.

pub mod anchor;
pub mod bonded;
pub mod cone;
pub mod counters;
pub mod cut;
pub mod extrude;
pub mod flat;
pub mod fold;
pub mod interval;
pub mod part;
pub mod pcurve;
pub mod pick;
pub mod place;
pub mod seam_frame;
pub mod unroll;
