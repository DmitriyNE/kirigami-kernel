//! Pure sewing checkers.
//!
//! The EDGE-OCCUPANCY four-bit `(A_L, A_R, B_L, B_R)` + frame-bit → row
//! classifier (Kani-exhaustive, ≤ 6 bits), the quadrant test (one cyclic
//! interval / all four / none; opposite quadrants ⇒ pinch, reject), the
//! mode-indexed identity dispatch (PAIR-IDENTICAL / OUTPUT-SOURCE-IDENTICAL),
//! `ε_φ` as the order sign of the monotone correspondence (never the derivative
//! sign), EDGE-EMB / EDGE-EDGE verdict logic, and SEW-LINK comparison over V_∂.
//! Implemented at M5. The sewing construction lives in the `sew` crate.
