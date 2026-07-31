//! Canonical x-monotone decomposition (M3a Phase 2; pending-v0.25 profile). Split
//! every circle/arc into simple x-monotone pieces at its exact x-extremal points
//! (`cx ± √r²`); the axis-aligned tag chart makes the half-angle pole the x-min
//! extremal, so extremal splitting subsumes pole splitting; no `Edge` spans more
//! than one simple arc; winding stays provenance on the source. Corpus:
//! `cx_full_circle_edge`.
