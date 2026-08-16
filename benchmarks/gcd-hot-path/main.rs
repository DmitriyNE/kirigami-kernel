//! OPT.3 step 0 — how much of `gcd_u128` can be bought back, on the *measured* operand mix.
//!
//! Harvested from a real `scale_probe` run (168M calls): 84.7% of calls have a power-of-two
//! operand, 76.5% have both operands under 2^64, mean 11.98 Euclidean iterations.
//!
//!   rustc -O benchmarks/gcd-hot-path/main.rs -o /tmp/gcdbench && /tmp/gcdbench

use std::time::Instant;

/// The shipping implementation: textbook Euclidean over `u128`.
fn gcd_current(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// (B) Power-of-two fast path, then the same loop. `gcd(a, 2^k) = 2^min(v2(a), k)`.
fn gcd_pow2(a: u128, b: u128) -> u128 {
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    if b & (b - 1) == 0 {
        return 1u128 << a.trailing_zeros().min(b.trailing_zeros());
    }
    if a & (a - 1) == 0 {
        return 1u128 << a.trailing_zeros().min(b.trailing_zeros());
    }
    gcd_current(a, b)
}

/// (C) Power-of-two fast path, then Euclidean narrowed to `u64` once both operands fit —
/// ARM64 has a hardware 64-bit divide but no 128-bit one.
fn gcd_pow2_u64(a: u128, b: u128) -> u128 {
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    if (b & (b - 1) == 0) || (a & (a - 1) == 0) {
        return 1u128 << a.trailing_zeros().min(b.trailing_zeros());
    }
    let (mut x, mut y) = (a, b);
    while y != 0 {
        if x <= u64::MAX as u128 && y <= u64::MAX as u128 {
            let (mut p, mut q) = (x as u64, y as u64);
            while q != 0 {
                let t = p % q;
                p = q;
                q = t;
            }
            return p as u128;
        }
        let t = x % y;
        x = y;
        y = t;
    }
    x
}

/// (D) Strip the common power of two, then Euclidean on the odd parts:
/// `gcd(2^i·m, 2^j·n) = 2^min(i,j)·gcd(m, n)` for odd `m`, `n`.
///
/// This is the shape worth preferring: the power-of-two case is not a special case at all but a
/// consequence (a power of two has odd part 1, so the Euclidean call returns immediately), and the
/// whole thing is one standard identity to state in Lean rather than a pile of branches.
fn gcd_strip2(a: u128, b: u128) -> u128 {
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    let (ia, ib) = (a.trailing_zeros(), b.trailing_zeros());
    let shift = ia.min(ib);
    let (mut x, mut y) = (a >> ia, b >> ib);
    if x == 1 || y == 1 {
        return 1u128 << shift;
    }
    while y != 0 {
        if x <= u64::MAX as u128 && y <= u64::MAX as u128 {
            let (mut p, mut q) = (x as u64, y as u64);
            while q != 0 {
                let t = p % q;
                p = q;
                q = t;
            }
            return (p as u128) << shift;
        }
        let t = x % y;
        x = y;
        y = t;
    }
    x << shift
}

/// (E) Pure Stein / binary GCD: shifts, comparisons and subtraction only — **no division at all**,
/// which is the appeal on a target whose 128-bit divide is a software routine.
fn gcd_stein(mut a: u128, mut b: u128) -> u128 {
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    let shift = a.trailing_zeros().min(b.trailing_zeros());
    a >>= a.trailing_zeros();
    loop {
        b >>= b.trailing_zeros();
        if a > b {
            core::mem::swap(&mut a, &mut b);
        }
        b -= a;
        if b == 0 {
            return a << shift;
        }
    }
}

/// (F) Stein with the trivial odd-part exit the harvested mix makes dominant. A power-of-two
/// operand has odd part 1, and pure Stein still walks ~bit-length iterations to discover that,
/// where one comparison settles it.
fn gcd_stein_fast1(a: u128, b: u128) -> u128 {
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    let (ia, ib) = (a.trailing_zeros(), b.trailing_zeros());
    let shift = ia.min(ib);
    let (mut x, mut y) = (a >> ia, b >> ib);
    if x == 1 || y == 1 {
        return 1u128 << shift;
    }
    loop {
        y >>= y.trailing_zeros();
        if x > y {
            core::mem::swap(&mut x, &mut y);
        }
        y -= x;
        if y == 0 {
            return x << shift;
        }
    }
}

/// (G) Strip the twos with an explicit **loop** instead of `trailing_zeros`.
///
/// Motivation is verification, not speed: `u128::trailing_zeros` is an intrinsic Aeneas cannot
/// model, so it lifts as an `axiom` and would need a hand-written faithful `def` — growing the
/// `lattice` model's audited TCB surface. A plain loop lifts natively and is provable with the
/// same `loop.spec_decr_nat` machinery already used for the Euclidean loop. The question is what
/// it costs.
fn gcd_strip2_loop(a: u128, b: u128) -> u128 {
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    let (mut x, mut y) = (a, b);
    let mut shift = 0u32;
    while x & 1 == 0 && y & 1 == 0 {
        x >>= 1;
        y >>= 1;
        shift += 1;
    }
    while x & 1 == 0 {
        x >>= 1;
    }
    while y & 1 == 0 {
        y >>= 1;
    }
    while y != 0 {
        if x <= u64::MAX as u128 && y <= u64::MAX as u128 {
            let (mut p, mut q) = (x as u64, y as u64);
            while q != 0 {
                let t = p % q;
                p = q;
                q = t;
            }
            return (p as u128) << shift;
        }
        let t = x % y;
        x = y;
        y = t;
    }
    x << shift
}

/// Operand pairs matching the harvested mix.
fn corpus(n: usize) -> Vec<(u128, u128)> {
    let mut s: u128 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let r = next();
        let pct = (r % 100) as u32;
        if pct < 85 {
            // power-of-two operand: dyadic denominators (2^30 / 2^50 grids)
            let k = 20 + (r >> 8) % 60;
            let other = (next() >> (r % 40)) | 1;
            if i % 2 == 0 {
                v.push((other, 1u128 << k));
            } else {
                v.push((1u128 << k, other));
            }
        } else if pct < 95 {
            // general, both under 2^64
            v.push(((next() >> 64) | 1, (next() >> 64) | 1));
        } else {
            // general, full width
            v.push((next() | 1, next() | 1));
        }
    }
    v
}

fn main() {
    let data = corpus(2_000_000);

    // Every candidate must agree with the shipping one on every pair.
    for &(a, b) in data.iter().take(200_000) {
        let want = gcd_current(a, b);
        assert_eq!(gcd_pow2(a, b), want, "pow2 disagrees on ({a}, {b})");
        assert_eq!(gcd_pow2_u64(a, b), want, "pow2+u64 disagrees on ({a}, {b})");
        assert_eq!(gcd_strip2(a, b), want, "strip2 disagrees on ({a}, {b})");
        assert_eq!(gcd_stein(a, b), want, "stein disagrees on ({a}, {b})");
        assert_eq!(gcd_stein_fast1(a, b), want, "stein_fast1 disagrees on ({a}, {b})");
        assert_eq!(gcd_strip2_loop(a, b), want, "strip2_loop disagrees on ({a}, {b})");
    }
    println!("agreement: ok (200k pairs)");

    let run = |name: &str, f: fn(u128, u128) -> u128| {
        let t = Instant::now();
        let mut acc = 0u128;
        for &(a, b) in &data {
            acc = acc.wrapping_add(f(a, b));
        }
        let el = t.elapsed().as_secs_f64();
        println!(
            "{name:14} {el:6.3}s  {:6.1} ns/call  (checksum {})",
            el / data.len() as f64 * 1e9,
            acc % 1000
        );
        el
    };

    let base = run("current", gcd_current);
    let b = run("pow2", gcd_pow2);
    let c = run("pow2+u64", gcd_pow2_u64);
    let d = run("strip2+u64", gcd_strip2);
    let g = run("strip2loop+u64", gcd_strip2_loop);
    let e = run("stein", gcd_stein);
    let f = run("stein+exit1", gcd_stein_fast1);
    println!(
        "speedup vs current: pow2 {:.2}x  pow2+u64 {:.2}x  strip2+u64 {:.2}x (SHIPPED)  stein {:.2}x  stein+exit1 {:.2}x",
        base / b, base / c, base / d, base / e, base / f
    );
    println!("stein+exit1 vs shipped: {:.2}x", d / f);
    println!(
        "strip2 via LOOP (no trailing_zeros intrinsic, no TCB growth): {:.2}x vs current, {:.2}x vs shipped",
        base / g,
        d / g
    );

    // Robustness: the 84.7% power-of-two share is a property of *this* device's dyadic grids. If a
    // future chart produced general denominators, would the choice of odd-part algorithm start to
    // matter? Re-run on a corpus with the power-of-two cases removed entirely.
    println!("\n-- general operands only (no power-of-two share) --");
    let mut s2: u128 = 0xD1B5_4A32_D192_ED03;
    let mut gen = Vec::with_capacity(1_000_000);
    for i in 0..1_000_000 {
        s2 ^= s2 << 13;
        s2 ^= s2 >> 7;
        s2 ^= s2 << 17;
        let a = if i % 3 == 0 { s2 >> 64 } else { s2 } | 3;
        s2 ^= s2 << 13;
        s2 ^= s2 >> 7;
        s2 ^= s2 << 17;
        let b = if i % 2 == 0 { s2 >> 64 } else { s2 } | 3;
        gen.push((a, b));
    }
    let run2 = |name: &str, f: fn(u128, u128) -> u128| {
        let t = Instant::now();
        let mut acc = 0u128;
        for &(a, b) in &gen {
            acc = acc.wrapping_add(f(a, b));
        }
        let el = t.elapsed().as_secs_f64();
        println!(
            "{name:14} {el:6.3}s  {:6.1} ns/call  (checksum {})",
            el / gen.len() as f64 * 1e9,
            acc % 1000
        );
        el
    };
    let g0 = run2("current", gcd_current);
    let g1 = run2("strip2+u64", gcd_strip2);
    let g2 = run2("stein+exit1", gcd_stein_fast1);
    println!(
        "general-only: strip2+u64 {:.2}x   stein+exit1 {:.2}x   (stein vs shipped {:.2}x)",
        g0 / g1,
        g0 / g2,
        g1 / g2
    );
}
