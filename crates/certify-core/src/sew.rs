//! Pure sewing checkers.
//!
//! The EDGE-OCCUPANCY four-bit `(A_L, A_R, B_L, B_R)` + frame-bit → row
//! classifier (Kani-exhaustive, ≤ 6 bits), the quadrant test (one cyclic
//! interval / all four / none; opposite quadrants ⇒ pinch, reject), the
//! mode-indexed identity dispatch (PAIR-IDENTICAL / OUTPUT-SOURCE-IDENTICAL),
//! EDGE-EMB / EDGE-EDGE verdict logic, and SEW-LINK comparison over V_∂.
//! Implemented at M5. The sewing construction lives in the `sew` crate.
//!
//! **`ε_φ` and EDGE-OCCUPANCY are minted upstream, at M4.** The order sign of the
//! monotone correspondence — the order sign, never the derivative sign — is minted by
//! [`crate::miter::eps_phi`] as part of MITER-EDGE-LEDGER, and the four-bit occupancy is
//! materialized there as [`crate::miter::Occupancy`]. SEW **consumes** both (it does not
//! re-mint them): the spec lists `ε_φ` under both the M4 ledger row and this M5 sewing row,
//! and the tiebreaker is ownership — M4 mints, M5 reads.
