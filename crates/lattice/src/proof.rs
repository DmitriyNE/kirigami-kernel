//! Kani bounded-model-checking harnesses for the L0 fast path (`small`), per
//! `vv-guide §5/§8`. Compiled only under `cargo kani` (`#[cfg(kani)]` in `lib.rs`).
//!
//! ## What Kani proves fast, and what is covered another way
//!
//! CBMC must fully unwind the `u128` Euclid gcd loop, and 128-bit division/modulo
//! bit-blasts into a problem that is slow for *every* backend (cadical, z3, …) —
//! the "Kani tractability of the gcd loop" risk. So the split is:
//!
//! - **Panic-/overflow-freedom of the loop-free ops** (`neg`, `sign`, `cmp` — only
//!   `checked_*` arithmetic, never a division by the denominator) is proven here
//!   over the **full i128 domain, including i128::MIN**, by constructing
//!   `SmallRat` directly (no `reduce` ⇒ no gcd loop). These solve in seconds and
//!   are the runnable Kani suite (CI runs them by name).
//! - **fast ≡ slow correctness of the gcd-carrying ops** (`add`/`sub`/`mul`, whose
//!   result goes through `reduce`) is established by the native exhaustive sweep
//!   `rat::tests::fast_path_small_grid_exhaustive` (every |·| ≤ 24 pair, checked
//!   against the i128 cross-multiply) plus the full-range differential
//!   (`rat::differential`, fast vs the real BigInt path). The `*_correct_i16`
//!   harnesses below are the SYMBOLIC version of that proof; they are correct but
//!   CBMC-expensive. Per the settled tool-fit decision, gcd/reduce *correctness*
//!   is owned by Lean at the task-3 spike (BMC is the wrong tool for iterative
//!   number theory); Kani keeps the gcd-free bridge + panic-freedom. No algorithm
//!   bandage (e.g. binary-GCD) — that would only fix this one operation.

use crate::small::{self, SmallRat, gcd_u128};

/// `gcd(|a|,|b|) == 1` — the reduced-form invariant (`0/1` counts: `gcd(0,1)=1`).
fn coprime(a: i128, b: i128) -> bool {
    gcd_u128(a.unsigned_abs(), b.unsigned_abs()) == 1
}

/// A reduced `SmallRat` from i16-range parts. On this domain `reduce` always
/// succeeds (magnitudes ≤ 2^15 fit; `d != 0`), so `.unwrap()` is provably safe.
fn reduced_i16(n: i16, d: i16) -> SmallRat {
    SmallRat::reduce(n as i128, d as i128).unwrap()
}

// ===========================================================================
// FAST — panic-/overflow-freedom of the loop-free ops over the FULL i128 domain
// (incl. i128::MIN). Constructed directly (no `reduce` ⇒ no gcd loop): seconds.
// A superset of valid inputs, so it also covers every reachable fast-path value.
// ===========================================================================

#[kani::proof]
fn neg_sign_panic_free_full_domain() {
    let x = SmallRat {
        num: kani::any(),
        den: kani::any(),
    };
    // sign: `signum` — total, never panics.
    let s = small::sign(&x);
    assert!(s == x.num.signum() as i8);
    // neg: `checked_neg` — `None` on i128::MIN, never a panic; den unchanged.
    if let Some(r) = small::neg(&x) {
        assert!(r.den == x.den);
    }
}

#[kani::proof]
fn cmp_panic_free_full_domain() {
    let x = SmallRat {
        num: kani::any(),
        den: kani::any(),
    };
    let y = SmallRat {
        num: kani::any(),
        den: kani::any(),
    };
    // cmp: `checked_mul` of the cross products — `None` on overflow, never panics.
    let _ = small::cmp(&x, &y);
}

// The Sturm sign-variation counter (vv-guide §5; the task-3 spike's function #1).
// Finite: an exhaustive check that the streaming counter matches an independent
// compact-then-pairwise reference over every {-1,0,1} sequence, and that the
// count is bounded (panic-/overflow-free). `unwind(7)` bounds the length-6
// sequence loops (passing `[i8;6]` as `&[i8]` hides the length from CBMC).
#[kani::proof]
#[kani::unwind(7)]
fn sign_variations_matches_reference() {
    let s: [i8; 6] = kani::any();
    for &x in &s {
        kani::assume((-1..=1).contains(&x));
    }
    let v = crate::sturm::sign_variations(&s);
    // reference: compact out zeros, then count adjacent differing signs.
    let mut buf = [0i8; 6];
    let mut k = 0usize;
    for &x in &s {
        if x != 0 {
            buf[k] = x;
            k += 1;
        }
    }
    let mut r = 0u32;
    for i in 1..k {
        if buf[i] != buf[i - 1] {
            r += 1;
        }
    }
    assert!(v == r);
    assert!(v <= 5);
}

// ===========================================================================
// SLOW (CBMC-expensive; spike-hardened) — fast ≡ slow (exact) + reduced-
// canonicalization + panic-freedom, over the i16-coordinate domain where i128 is
// an exact wide reference and the fast path never promotes. Correct, but the gcd
// loop makes these impractical to run in bulk today; the native
// `fast_path_small_grid_exhaustive` covers the same ground quickly.
// ===========================================================================

#[kani::proof]
#[kani::unwind(50)]
fn add_correct_i16_domain() {
    let (xn, xd) = (kani::any::<i16>(), kani::any::<i16>());
    let (yn, yd) = (kani::any::<i16>(), kani::any::<i16>());
    kani::assume(xd != 0 && yd != 0);
    let x = reduced_i16(xn, xd);
    let y = reduced_i16(yn, yd);
    let r = small::add(&x, &y).unwrap(); // never overflows on i16 ⇒ never promotes
    assert!(r.den > 0 && coprime(r.num, r.den));
    assert!(r.num * (x.den * y.den) == (x.num * y.den + y.num * x.den) * r.den);
}

#[kani::proof]
#[kani::unwind(50)]
fn mul_correct_i16_domain() {
    let (xn, xd) = (kani::any::<i16>(), kani::any::<i16>());
    let (yn, yd) = (kani::any::<i16>(), kani::any::<i16>());
    kani::assume(xd != 0 && yd != 0);
    let x = reduced_i16(xn, xd);
    let y = reduced_i16(yn, yd);
    let r = small::mul(&x, &y).unwrap();
    assert!(r.den > 0 && coprime(r.num, r.den));
    assert!(r.num * (x.den * y.den) == (x.num * y.num) * r.den);
}

#[kani::proof]
#[kani::unwind(40)]
fn reduce_no_panic_num_full() {
    let n = kani::any::<i128>(); // full range: covers i128::MIN in unsigned_abs / neg_mag
    let d = kani::any::<i16>() as i128; // bounded so gcd(huge, small) is short
    kani::assume(d != 0);
    if let Some(r) = SmallRat::reduce(n, d) {
        assert!(r.den > 0);
        assert!(coprime(r.num, r.den));
    }
}
