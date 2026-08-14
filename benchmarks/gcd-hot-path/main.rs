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
    println!(
        "speedup: pow2 {:.2}x   pow2+u64 {:.2}x   strip2+u64 {:.2}x",
        base / b,
        base / c,
        base / d
    );
}
