#![forbid(unsafe_code)]
//! `gate` — records + validity (shell tier; M6).
//!
//! The certificate store (append-only, provenance-linked, FRESH promotion) and
//! the orchestration that evaluates CLOSURE-CAP / CLOSURE_VALID /
//! VALID_material / VALID_solid-closure over it. The *pure* verdict-propagation
//! algebra those evaluations reduce to lives in `certify_core::gate`; this crate
//! is the stateful shell around it. VALID_complement is evaluated over the
//! clipped domains `D^closure` where clips exist.

pub mod store;
