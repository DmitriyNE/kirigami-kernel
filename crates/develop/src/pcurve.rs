//! **Parametric domain curves** (`p-curves`) — a curve `t ↦ (σ(t), µ̂(t))` in a chart's own
//! `(σ, µ̂)` parameter plane, the general shape of a cut curve on a ruled sheet.
//!
//! The trim layer's original rails are *graphs*: `µ̂ = f(σ)`, one ruling coordinate per σ. That
//! models a σ-sliced band's *boundary* well, but it cannot model a curve that **turns around in
//! σ** — and every closed cut does. Where a solid cutter grazes the sheet tangentially, the cut
//! reverses direction and `dµ̂/dσ` blows up; a graph has to stop short of that turning point, so
//! a closed hole must be split into two graphs (near/far) joined by straight bridges, whose size
//! has a floor no fit quality can lower (measured: ~30% of the hole's height at best). A p-curve
//! carries its own parameter, passes through the turning points, and closes.
//!
//! Nothing below this module needed to change to accept one: the development's chord certificate
//! [`anchor_dev`](crate::anchor::anchor_dev) has always taken `σ(t)` and `µ̂(t)` over a `t`-span —
//! the unroll simply instantiated it with the identity reparametrization. A graph is the special
//! case [`PCurve::graph`] (`σ(t) = t`), so both live in one vocabulary.
//!
//! ```
//! use develop::pcurve::PCurve;
//! use lattice::{Bignum, Interval, Poly, Rat, RatFunc};
//!
//! // The unit circle as a p-curve: σ(t) = (1−t²)/(1+t²), µ̂(t) = 2t/(1+t²) — a curve that
//! // turns around in σ at t = 0, which no graph µ̂ = f(σ) can represent in one piece.
//! let den = Poly::<Bignum>::from_coeffs(vec![Rat::from_i128(1), Rat::from_i128(0), Rat::from_i128(1)]);
//! let c = PCurve {
//!     sigma: RatFunc::new(Poly::from_coeffs(vec![Rat::from_i128(1), Rat::from_i128(0), Rat::from_i128(-1)]), den.clone()),
//!     mu: RatFunc::new(Poly::from_coeffs(vec![Rat::from_i128(0), Rat::from_i128(2)]), den),
//!     domain: Interval { lo: Rat::from_i128(-1), hi: Rat::from_i128(1) },
//! };
//! let p = c.eval(&Rat::from_i128(0)).unwrap();
//! assert_eq!(p, [Rat::from_i128(1), Rat::from_i128(0)]);
//! // σ turns around inside the domain — the graph model's blind spot.
//! assert_eq!(c.sigma_turning_points(64, 40).unwrap().len(), 1);
//! ```

use crate::interval::{RatIv, eval_ratfunc_on};
use lattice::{Backend, Bignum, Interval, Poly, Rat, RatFunc};

/// Bisect `f` for a sign change on `[lo, hi]` (needs `f(lo)·f(hi) < 0`), returning a rational in
/// the final bracket — exact sign evaluation, rational midpoints. `None` if there is no sign
/// change or a pole is hit.
pub fn bisect_root<B: Backend>(
    f: &RatFunc<B>,
    lo: &Rat<B>,
    hi: &Rat<B>,
    iters: usize,
) -> Option<Rat<B>> {
    let half = Rat::new(1, 2);
    let mut a = lo.clone();
    let mut b = hi.clone();
    let sa = f.eval(&a)?.sign();
    if sa == 0 {
        return Some(a);
    }
    if f.eval(&b)?.sign() == sa {
        return None;
    }
    for _ in 0..iters {
        let m = a.add(&b).mul(&half);
        let sm = f.eval(&m)?.sign();
        if sm == 0 {
            return Some(m);
        }
        if sm == sa {
            a = m; // sign at `a` unchanged
        } else {
            b = m;
        }
    }
    Some(a.add(&b).mul(&half))
}

/// All roots of `f` on `[lo, hi]`, in order, found by scanning `scan` sub-intervals: a node where
/// `f` vanishes **exactly** is a root as it stands, and each bracketing sign change is bisected.
/// `None` if `f` has a pole at a scan node.
///
/// Taking the exact hit is not a nicety: symmetric geometry puts roots precisely on dyadic scan
/// nodes (a curve turning at `t = 0`, a hole centred on a ruling), and a scan that only looks for
/// sign *changes* steps over them — the flanking cells each have a zero endpoint, so neither
/// registers as a change. A root the scan straddles evenly (a double root, or two closer than one
/// cell) remains invisible; the caller owns the scan density, and consumers are fail-closed on a
/// miscount.
pub fn scan_roots<B: Backend>(
    f: &RatFunc<B>,
    lo: &Rat<B>,
    hi: &Rat<B>,
    scan: usize,
    iters: usize,
) -> Option<Vec<Rat<B>>> {
    let n = scan.max(4);
    let width = hi.sub(lo).div(&Rat::from_i128(n as i128));
    let mut prev: Option<(Rat<B>, i8)> = None;
    let mut roots: Vec<Rat<B>> = Vec::new();
    for k in 0..=n {
        let x = lo.add(&width.mul(&Rat::from_i128(k as i128)));
        let s = f.eval(&x)?.sign();
        if s == 0 {
            roots.push(x.clone());
        } else if let Some((px, ps)) = &prev {
            if *ps != 0 && *ps != s {
                roots.push(bisect_root(f, px, &x, iters)?);
            }
        }
        prev = Some((x, s));
    }
    Some(roots)
}

/// A curve in a chart's `(σ, µ̂)` domain, parametrized rationally: `t ↦ (σ(t), µ̂(t))` over
/// `domain`. Free to turn around in σ (see the module docs); a graph rail is the special case
/// [`PCurve::graph`].
pub struct PCurve<B: Backend = Bignum> {
    /// The chart σ-coordinate as a rational function of the curve parameter.
    pub sigma: RatFunc<B>,
    /// The ruling coordinate as a rational function of the curve parameter.
    pub mu: RatFunc<B>,
    /// The parameter span the curve is authored over.
    pub domain: Interval<B>,
}

// Hand-written so `B` need not be `Clone` (the backend markers are not).
impl<B: Backend> Clone for PCurve<B> {
    fn clone(&self) -> Self {
        PCurve {
            sigma: self.sigma.clone(),
            mu: self.mu.clone(),
            domain: self.domain.clone(),
        }
    }
}

impl<B: Backend> PCurve<B> {
    /// The **graph** p-curve `σ(t) = t`, `µ̂(t) = f(t)` — the trim layer's original rail shape,
    /// expressed in the general vocabulary so graphs and turning curves compose in one loop.
    pub fn graph(mu: RatFunc<B>, span: Interval<B>) -> Self {
        PCurve {
            sigma: RatFunc::from_poly(Poly::from_coeffs(vec![
                Rat::from_i128(0),
                Rat::from_i128(1),
            ])),
            mu,
            domain: span,
        }
    }

    /// The domain point at a rational parameter, or `None` on a coefficient pole.
    pub fn eval(&self, t: &Rat<B>) -> Option<[Rat<B>; 2]> {
        Some([self.sigma.eval(t)?, self.mu.eval(t)?])
    }

    /// The domain **box** enclosing the curve over a parameter interval — the enclosure every
    /// certificate above this module consumes. `None` if either component denominator straddles
    /// zero on the interval (a pole risk, so the enclosure would be unbounded).
    pub fn eval_on(&self, t: &RatIv<B>) -> Option<[RatIv<B>; 2]> {
        Some([
            eval_ratfunc_on(&self.sigma, t)?,
            eval_ratfunc_on(&self.mu, t)?,
        ])
    }

    /// The parameters **strictly inside** the domain where σ reverses direction — the zeros of
    /// `σ′(t)`. These are exactly the points a graph `µ̂ = f(σ)` cannot represent, and the points
    /// a σ-sliced consumer must treat as extremal. `None` on a pole at a scan node.
    ///
    /// Endpoints are excluded deliberately: a curve that merely *arrives* at an extremum at the
    /// end of its span does not reverse within the span, and counting it would make
    /// [`split_at_turns`](PCurve::split_at_turns) recur forever, since each split creates such an
    /// endpoint.
    pub fn sigma_turning_points(&self, scan: usize, iters: usize) -> Option<Vec<Rat<B>>> {
        use core::cmp::Ordering;
        let roots = scan_roots(
            &self.sigma.derivative(),
            &self.domain.lo,
            &self.domain.hi,
            scan,
            iters,
        )?;
        Some(
            roots
                .into_iter()
                .filter(|t| {
                    self.domain.lo.cmp(t) == Ordering::Less
                        && t.cmp(&self.domain.hi) == Ordering::Less
                })
                .collect(),
        )
    }

    /// Split the curve at every interior σ-turning point, yielding σ-**monotone** pieces in
    /// parameter order — the form a σ-indexed consumer (the sliced solid builder, a graph-shaped
    /// rail chain) can take. A curve with no interior turn comes back whole.
    pub fn split_at_turns(&self, scan: usize, iters: usize) -> Option<Vec<Self>> {
        let turns = self.sigma_turning_points(scan, iters)?;
        let mut out = Vec::with_capacity(turns.len() + 1);
        let mut lo = self.domain.lo.clone();
        for t in turns {
            out.push(self.restrict(&Interval {
                lo: lo.clone(),
                hi: t.clone(),
            })?);
            lo = t;
        }
        out.push(self.restrict(&Interval {
            lo,
            hi: self.domain.hi.clone(),
        })?);
        Some(out)
    }

    /// The parameters in the domain where the curve crosses the ruling `σ = s`, in order — the
    /// primitive a σ-sliced solid builder needs to clip a curve at a station (a turning curve can
    /// cross the same station more than once, which is why this returns a list). `None` on a pole
    /// at a scan node.
    pub fn params_at_sigma(&self, s: &Rat<B>, scan: usize, iters: usize) -> Option<Vec<Rat<B>>> {
        let shifted = self
            .sigma
            .sub(&RatFunc::from_poly(Poly::constant(s.clone())));
        scan_roots(&shifted, &self.domain.lo, &self.domain.hi, scan, iters)
    }

    /// The same curve restricted to a sub-span, or `None` if the span is not a non-degenerate
    /// sub-interval of the domain.
    pub fn restrict(&self, span: &Interval<B>) -> Option<Self> {
        use core::cmp::Ordering;
        if span.lo.cmp(&span.hi) != Ordering::Less
            || span.lo.cmp(&self.domain.lo) == Ordering::Less
            || self.domain.hi.cmp(&span.hi) == Ordering::Less
        {
            return None;
        }
        Some(PCurve {
            sigma: self.sigma.clone(),
            mu: self.mu.clone(),
            domain: span.clone(),
        })
    }

    /// Split at an interior parameter into the two sub-curves, or `None` if `t` is not strictly
    /// inside the domain.
    pub fn split_at(&self, t: &Rat<B>) -> Option<(Self, Self)> {
        let lo = Interval {
            lo: self.domain.lo.clone(),
            hi: t.clone(),
        };
        let hi = Interval {
            lo: t.clone(),
            hi: self.domain.hi.clone(),
        };
        Some((self.restrict(&lo)?, self.restrict(&hi)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::Poly;

    type Q = Rat<Bignum>;

    fn poly(cs: &[i128]) -> Poly<Bignum> {
        Poly::from_coeffs(cs.iter().map(|&c| Q::from_i128(c)).collect())
    }

    /// The rational half-turn `σ = (1−t²)/(1+t²)`, `µ̂ = 2t/(1+t²)`: the unit circle traced
    /// through its rightmost point. σ turns around at `t = 0`, exactly the configuration that
    /// forces the graph model to split a hole in two and bridge the gap.
    fn circle() -> PCurve<Bignum> {
        let den = poly(&[1, 0, 1]);
        PCurve {
            sigma: RatFunc::new(poly(&[1, 0, -1]), den.clone()),
            mu: RatFunc::new(poly(&[0, 2]), den),
            domain: Interval {
                lo: Q::from_i128(-1),
                hi: Q::from_i128(1),
            },
        }
    }

    #[test]
    fn a_turning_curve_reports_its_turning_point() {
        let c = circle();
        let turns = c.sigma_turning_points(64, 40).expect("no pole");
        assert_eq!(turns.len(), 1, "the circle turns around in σ exactly once");
        // σ′ = −4t/(1+t²)², so the turn sits at t = 0 (exactly, by the bisection's zero hit).
        assert_eq!(turns[0].sign(), 0);
        // And it is the σ-extremum: σ(0) = 1 is larger than either end's σ(±1) = 0.
        assert_eq!(c.eval(&turns[0]).unwrap()[0], Q::from_i128(1));
    }

    #[test]
    fn a_station_crossed_twice_is_reported_twice() {
        let c = circle();
        // σ = 1/2 cuts the traced arc on both sides of the turning point.
        let ts = c.params_at_sigma(&Q::new(1, 2), 64, 40).expect("no pole");
        assert_eq!(ts.len(), 2, "a turning curve crosses one station twice");
        for t in &ts {
            let p = c.eval(t).unwrap();
            // On the circle: σ² + µ̂² = 1, and the crossing really is at σ ≈ 1/2.
            let tiny = Q::new(1, 1_000_000);
            let near = |a: &Q, b: &Q| {
                let d = a.sub(b);
                let d = if d.sign() < 0 {
                    Q::from_i128(0).sub(&d)
                } else {
                    d
                };
                d.cmp(&tiny) == core::cmp::Ordering::Less
            };
            let r = p[0].mul(&p[0]).add(&p[1].mul(&p[1]));
            assert!(
                near(&r, &Q::from_i128(1)),
                "the crossing lies on the circle"
            );
            assert!(near(&p[0], &Q::new(1, 2)), "and at the requested station");
        }
    }

    #[test]
    fn the_enclosure_contains_the_traced_points() {
        let c = circle();
        let iv = RatIv::new(Q::new(-1, 4), Q::new(1, 4));
        let [sig, mu] = c.eval_on(&iv).expect("no pole");
        for t in [
            Q::new(-1, 4),
            Q::new(-1, 8),
            Q::from_i128(0),
            Q::new(1, 8),
            Q::new(1, 4),
        ] {
            let p = c.eval(&t).unwrap();
            assert!(sig.contains(&p[0]), "σ enclosure must contain σ(t)");
            assert!(mu.contains(&p[1]), "µ̂ enclosure must contain µ̂(t)");
        }
    }

    #[test]
    fn a_graph_is_the_identity_reparametrized_special_case() {
        let span = Interval {
            lo: Q::from_i128(0),
            hi: Q::from_i128(1),
        };
        let f = RatFunc::from_poly(poly(&[1, 2])); // µ̂ = 1 + 2σ
        let g = PCurve::graph(f, span);
        assert_eq!(g.eval(&Q::new(1, 4)).unwrap(), [Q::new(1, 4), Q::new(3, 2)]);
        // A graph never turns around — that is exactly its limitation, stated as a property.
        assert!(g.sigma_turning_points(64, 40).unwrap().is_empty());
    }

    #[test]
    fn restrict_and_split_respect_the_domain() {
        let c = circle();
        let (a, b) = c.split_at(&Q::from_i128(0)).expect("interior split");
        assert_eq!(a.domain.hi, Q::from_i128(0));
        assert_eq!(b.domain.lo, Q::from_i128(0));
        // Each half is now σ-monotone: the split at the turning point is what makes a turning
        // curve safe for a σ-indexed consumer.
        assert!(a.sigma_turning_points(64, 40).unwrap().is_empty());
        assert!(b.sigma_turning_points(64, 40).unwrap().is_empty());
        // And that split is what `split_at_turns` does on its own — the form the sliced solid
        // builder consumes.
        let pieces = c.split_at_turns(64, 40).expect("no pole");
        assert_eq!(pieces.len(), 2);
        assert!(
            pieces
                .iter()
                .all(|p| p.sigma_turning_points(64, 40).unwrap().is_empty()),
            "every piece must be σ-monotone"
        );
        // Outside the domain is refused, not clamped.
        assert!(
            c.restrict(&Interval {
                lo: Q::from_i128(-2),
                hi: Q::from_i128(0)
            })
            .is_none()
        );
    }
}
