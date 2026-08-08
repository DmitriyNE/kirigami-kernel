// UI fixture for the `no_float` dylint lint. Named `lattice.rs` so the test crate's name
// matches a certified crate (the lint scopes to lattice/certify_core/arrange2d).
//
// Covers both things the lint must catch: float LITERALS (`check_expr`) and float TYPES
// (`check_ty`) — the latter is the coverage the retired `cargo xtask lint` token scan had.
#![allow(dead_code)]

// --- float literals (check_expr): no `f32`/`f64` token, invisible to a text scan ---
fn literals() {
    let _x = 1.5;
    let _y = 2.0 + 3.0;
}

// --- float types (check_ty): fn signature — param and return, no literal in the body ---
fn sig(w: f64) -> f64 {
    w
}

// --- float type: struct field ---
struct HasField {
    x: f32,
}

// --- float type: generic argument `Vec<f64>` ---
fn generic() -> Vec<f64> {
    Vec::new()
}

// --- float type: `as` cast target ---
fn cast(n: i64) -> i64 {
    let _z = n as f64;
    n
}

// --- float type via a type-relative path: `f64::EPSILON` ---
fn assoc() {
    let _e = f64::EPSILON;
}

fn main() {}
