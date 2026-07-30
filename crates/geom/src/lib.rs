#![forbid(unsafe_code)]
//! `geom` — chart primitives (shell tier; M1).
//!
//! Quaternion splines `q(σ)`, h-splines, `C(σ, μ, w)`, `n`/`r`/pedal, primitive
//! tags, and the hatted stall calculus (`p̂, μ̂, r̂, n̂′, Ĵ`) with the tested
//! identity `J_raw = p̂·Ĵ` (a positive factor — if you see `/p` where `/p̂` is
//! meant, that is the fossil bug). Substitution/removability transport, the
//! `b_J`/`b_i`/`G_i` fields, and `N_i^cut`. The two device fixtures (cone; petal
//! conical flank) are data in the `fixtures` crate.
