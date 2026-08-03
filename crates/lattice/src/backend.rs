//! The exact-arithmetic backend trait — the `no_std` + `alloc` boundary.
//!
//! The concrete bignum backend was chosen by benchmark
//! (`docs/lattice-backend-benchmark.md`: dashu). Every consumer goes through this
//! trait, so the backend stays swappable without touching callers and the `no_std`
//! constraint is a property of this API rather than of the underlying crate.
//!
//! The trait abstracts **only the BigInt slow path** — the semantic reference
//! whose results match Lean's `Int`/`Rat` (`vv-guide §4`). The L0 fixed-limb
//! fast path is not part of the trait; it is the lattice-level [`crate::Rat`] /
//! [`crate::Int`] two-tier dispatch built on top (`rat` module), and its
//! equivalence to this slow path on the fast domain is Kani-verified.
//!
//! The surface is deliberately minimal: constructors, add/sub/mul/neg, exact
//! cmp/sign/gcd, is_zero, numerator/denominator. Higher-level operations
//! (polynomial arithmetic, Sturm, resultants, interval-plus-separation) are built
//! on top rather than added here; the minimal `Clone + Eq` associated bounds leave
//! room for them without churn.

use core::cmp::Ordering;

/// Exact integer + rational arithmetic. `Int`/`Rat` are opaque associated types
/// (the concrete backend wraps them in newtypes), total and panic-free. `Rat` is
/// always kept in canonical form: reduced to lowest terms with denominator `> 0`.
///
/// The trait's methods are the *slow path*. Rational `cmp`/`sign` are **total and
/// exact** — they never return an "unresolved" verdict; so is the algebraic
/// (√-carrying) comparison in `algebraic` (spec §8.1 "A/1D inequalities: total").
/// The three-valued `Verdict` middle is certify-core's A/nD strict-sign concern,
/// not any lattice comparison.
pub trait Backend {
    /// Arbitrary-precision integer (BigInt slow path; matches Lean `Int`).
    type Int: Clone + Eq;
    /// Arbitrary-precision rational, reduced with denominator `> 0` (matches Lean `Rat`).
    type Rat: Clone + Eq;

    // ---- integer constructors ------------------------------------------------
    fn int_zero() -> Self::Int;
    fn int_one() -> Self::Int;
    fn int_from_i128(v: i128) -> Self::Int;
    /// Exact narrowing to `i128`, or `None` if it does not fit. Drives fast-path
    /// demotion in the two-tier [`crate::Int`]/[`crate::Rat`].
    fn int_try_to_i128(a: &Self::Int) -> Option<i128>;

    // ---- integer ops (total, exact) -----------------------------------------
    fn int_add(a: &Self::Int, b: &Self::Int) -> Self::Int;
    fn int_sub(a: &Self::Int, b: &Self::Int) -> Self::Int;
    fn int_mul(a: &Self::Int, b: &Self::Int) -> Self::Int;
    fn int_neg(a: &Self::Int) -> Self::Int;
    fn int_cmp(a: &Self::Int, b: &Self::Int) -> Ordering;
    /// `-1 | 0 | 1`.
    fn int_sign(a: &Self::Int) -> i8;
    fn int_is_zero(a: &Self::Int) -> bool;
    /// Greatest common divisor, always `≥ 0` (`gcd(0,0) = 0`).
    fn int_gcd(a: &Self::Int, b: &Self::Int) -> Self::Int;
    /// Least common multiple, always `≥ 0` (`lcm(_,0) = 0`). The §2.2
    /// "integers over the lcm of denominators" per-predicate rescale primitive.
    fn int_lcm(a: &Self::Int, b: &Self::Int) -> Self::Int;
    /// Truncated quotient and remainder: `a = q·b + r`, with `r` the sign of `a`
    /// and `|r| < |b|` (matches `i128` `/`,`%` and dashu `div_rem`). `b == 0` is
    /// out of contract, kept panic-free by convention (like [`Self::rat_from_ints`]).
    fn int_divrem(a: &Self::Int, b: &Self::Int) -> (Self::Int, Self::Int);
    /// Truncated quotient. Doubles as *exact* division where `b | a` (Bareiss,
    /// polynomial content), for which the truncation convention never bites.
    fn int_div(a: &Self::Int, b: &Self::Int) -> Self::Int {
        Self::int_divrem(a, b).0
    }
    /// Truncated remainder (sign of `a`).
    fn int_rem(a: &Self::Int, b: &Self::Int) -> Self::Int {
        Self::int_divrem(a, b).1
    }

    // ---- rational constructors (result is normalized: reduced, den > 0) ------
    fn rat_from_i128(v: i128) -> Self::Rat;
    /// Build `num/den`. `den == 0` is out of contract (§2.2 never feeds a zero
    /// denominator to a predicate); it is kept panic-free by convention
    /// (debug-assert + a defined zero fallback), never a runtime panic.
    fn rat_from_ints(num: Self::Int, den: Self::Int) -> Self::Rat;

    // ---- rational ops (total, exact, result stays normalized) ----------------
    fn rat_add(a: &Self::Rat, b: &Self::Rat) -> Self::Rat;
    fn rat_sub(a: &Self::Rat, b: &Self::Rat) -> Self::Rat;
    fn rat_mul(a: &Self::Rat, b: &Self::Rat) -> Self::Rat;
    /// Exact division `a / b`. `b == 0` is out of contract (panic-free by
    /// convention, like [`Self::rat_from_ints`]).
    fn rat_div(a: &Self::Rat, b: &Self::Rat) -> Self::Rat;
    fn rat_neg(a: &Self::Rat) -> Self::Rat;
    fn rat_cmp(a: &Self::Rat, b: &Self::Rat) -> Ordering;
    /// `-1 | 0 | 1`.
    fn rat_sign(a: &Self::Rat) -> i8;
    fn rat_is_zero(a: &Self::Rat) -> bool;

    // ---- reduced-form field access (den > 0) --------------------------------
    fn rat_numer(a: &Self::Rat) -> Self::Int;
    /// Always `> 0`.
    fn rat_denom(a: &Self::Rat) -> Self::Int;
}
