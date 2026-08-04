// UI fixture for the `no_float` dylint lint. Named `lattice.rs` so the test crate's name
// matches a certified crate (the lint scopes to lattice/certify_core/arrange2d).
fn main() {
    let _x = 1.5; // ~ NO_FLOAT: a float literal, invisible to the `f32`/`f64` token scan
    let _y = 2.0 + 3.0;
}
