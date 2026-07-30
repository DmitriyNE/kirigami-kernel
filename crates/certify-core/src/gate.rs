//! Pure gate algebra.
//!
//! VALID_complement / CLOSURE-CAP / CLOSURE_VALID / VALID_material /
//! VALID_solid-closure evaluated as pure functions over stored records, plus
//! the unresolved-propagation rules. Gate formulas contain only truth-valued
//! certificate expressions — no imperatives, no "band or fail" disjunct (spec
//! §8.2/§8.6). Implemented at M6. The append-only, provenance-linked, FRESH-
//! promoting certificate store lives in the `gate` shell crate.
