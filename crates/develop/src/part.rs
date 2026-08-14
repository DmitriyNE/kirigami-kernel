//! `part` — the development abstraction and the piecewise-support gluing.
//!
//! [`Development`] is the small trait the flat pipeline ([`unroll`](crate::unroll), later
//! [`fold`](crate::fold)) consumes instead of a concrete [`ConeDevelopment`]: **one pipeline, many
//! implementors** — the single-region cone, the piecewise-support gluing below, and (future) a
//! reparametrized or strain-budgeted development, none of which touch the consumers.
//!
//! [`PiecewiseDevelopment`] glues N support-regions that share one frame (`q` — equivalently the
//! angle coefficient `c` and `ρ²`; only the support `h` varies per region) into **one connected
//! flat development**: a running cumulative directrix `γ` carries each region's flat frame on from
//! the previous region's end, and every region integrates its own `γ` only over its own σ-window
//! (where its support is tame). This is the self-lapping demo's `gamma_grid`/`point_at` machinery
//! lifted into the engine.
//!
//! ```
//! use develop::cone::{ConeDevelopment, DevConfig};
//! use develop::part::{Development, PiecewiseDevelopment};
//! use fixtures::devices::cone_wrap;
//! use lattice::{Bignum, Interval, Rat};
//!
//! // A single-region gluing is just the (signed) cone development.
//! let dev = ConeDevelopment::new(&cone_wrap()).unwrap();
//! let pw = PiecewiseDevelopment::new(vec![(
//!     Interval { lo: Rat::from_i128(0), hi: Rat::from_i128(1) },
//!     ConeDevelopment::new(&cone_wrap()).unwrap(),
//! )])
//! .unwrap();
//! let cfg = DevConfig::<Bignum>::tight();
//! let (s, m) = (Rat::new(1, 3), Rat::from_i128(-1));
//! let glued = Development::point(&pw, &s, &m, &cfg).unwrap().center();
//! assert_eq!(glued, dev.point_signed(&s, &m, &cfg).center());
//! ```

use crate::anchor::AnchorFrame;
use crate::cone::{ConeDevelopment, DevConfig, FlatBox};
use crate::interval::RatIv;
use core::cell::RefCell;
use core::cmp::Ordering;
use lattice::{Backend, Bignum, Interval, Rat};

/// A certified development map `D(σ, µ̂)` — what the flat pipeline consumes.
///
/// The contract is **a certified enclosure of the implementor's canonical development**; exact
/// isometry and the µ̂-sign convention are *implementor properties*, not trait promises:
/// [`ConeDevelopment`] keeps its `|µ̂|` fast path on the apex cone (γ ≡ 0) and signed µ̂ with a
/// directrix, while [`PiecewiseDevelopment`] is **always signed** (the connected gluing's
/// requirement). The two conventions must not be mixed along one connected boundary — which is
/// exactly why the choice lives with the implementor and the pipeline stays agnostic.
pub trait Development<B: Backend> {
    /// The certified flat point `D(σ, µ̂)` — `None` on a pole.
    fn point(&self, sigma: &Rat<B>, mu_hat: &Rat<B>, cfg: &DevConfig<B>) -> Option<FlatBox<B>>;

    /// `D` over *interval* σ and µ̂ — the ANCHOR-side sup-bound primitive. `None` on a pole risk.
    fn point_on(
        &self,
        sigma: &RatIv<B>,
        mu_hat: &RatIv<B>,
        cfg: &DevConfig<B>,
    ) -> Option<FlatBox<B>>;

    /// A certified enclosure of the flat angle `ψ(σ)` over an interval σ.
    fn angle_on(&self, sigma: &RatIv<B>, terms: usize) -> RatIv<B>;

    /// A certified enclosure of the ruling-speed radius `ρ(σ)` over an interval σ.
    fn radius_on(&self, sigma: &RatIv<B>, eps: &Rat<B>) -> Option<RatIv<B>>;

    /// Whether the development carries a nonzero flat directrix (`γ ≠ 0`) anywhere.
    fn has_directrix(&self) -> bool;

    /// The single-region ANCHOR decomposition of a σ-span: sub-spans (split at region joins),
    /// each with the concrete cone development it rides and the piecewise frame (the running
    /// `base` plus from-`lo` γ) the [`anchor_dev`](crate::anchor::anchor_dev) checker develops
    /// it in. A single-region development is one frameless piece — the original checker path,
    /// byte-identical. `None` when the span leaves the development's σ-domain or γ poles.
    fn anchor_pieces(
        &self,
        span: &Interval<B>,
        cfg: &DevConfig<B>,
    ) -> Option<Vec<AnchorPiece<'_, B>>>;
}

/// One single-region piece of an anchored σ-span: the sub-span, the concrete development whose
/// window it lies in, and the piecewise frame (`None` = the plain single-region path).
pub struct AnchorPiece<'a, B: Backend = Bignum> {
    /// The σ-sub-span this piece covers (within one region's window).
    pub span: Interval<B>,
    /// The region's concrete cone development.
    pub dev: &'a ConeDevelopment<B>,
    /// The piecewise frame the checker develops in, or `None` for the plain path.
    pub frame: Option<AnchorFrame<B>>,
}

impl<B: Backend> Development<B> for ConeDevelopment<B> {
    fn point(&self, sigma: &Rat<B>, mu_hat: &Rat<B>, cfg: &DevConfig<B>) -> Option<FlatBox<B>> {
        Some(ConeDevelopment::point(self, sigma, mu_hat, cfg))
    }

    fn point_on(
        &self,
        sigma: &RatIv<B>,
        mu_hat: &RatIv<B>,
        cfg: &DevConfig<B>,
    ) -> Option<FlatBox<B>> {
        ConeDevelopment::point_on(self, sigma, mu_hat, cfg)
    }

    fn angle_on(&self, sigma: &RatIv<B>, terms: usize) -> RatIv<B> {
        ConeDevelopment::angle_on(self, sigma, terms)
    }

    fn radius_on(&self, sigma: &RatIv<B>, eps: &Rat<B>) -> Option<RatIv<B>> {
        ConeDevelopment::radius_on(self, sigma, eps)
    }

    fn has_directrix(&self) -> bool {
        ConeDevelopment::has_directrix(self)
    }

    fn anchor_pieces(
        &self,
        span: &Interval<B>,
        _cfg: &DevConfig<B>,
    ) -> Option<Vec<AnchorPiece<'_, B>>> {
        Some(vec![AnchorPiece {
            span: span.clone(),
            dev: self,
            frame: None,
        }])
    }
}

/// A piecewise-support developable: N regions sharing one frame, glued by the running cumulative
/// flat directrix into **one connected development** (see the module docs). Exact recipe data; the
/// certificates come from the per-region [`ConeDevelopment`]s it routes to.
pub struct PiecewiseDevelopment<B: Backend = Bignum> {
    /// σ-band → the region's development; validated sorted, contiguous, frame-shared.
    regions: Vec<(Interval<B>, ConeDevelopment<B>)>,
    /// Memoized cumulative-γ prefixes (the full-window integral of every region), keyed by the
    /// [`DevConfig`] that computed them — the certified quadrature is the dominant cost, and a
    /// consumer (unroll, anchor) calls [`cum_before`](Self::cum_before) once per *edge*.
    /// Enclosure reuse is sound (an enclosure is an enclosure); a changed budget recomputes.
    cum_cache: RefCell<Option<CumCache<B>>>,
}

/// The memoized cumulative-γ prefixes plus the budget that produced them.
struct CumCache<B: Backend> {
    terms: usize,
    sqrt_eps: Rat<B>,
    /// `cum[k]` = Σ over regions `< k` of the full-window γ integral.
    cum: Vec<[RatIv<B>; 2]>,
}

impl<B: Backend> PiecewiseDevelopment<B> {
    /// Glue the regions, or `None` unless the bands **tile** (each `lo < hi`, consecutive bands
    /// meet exactly — no gap, no overlap) and every region shares the first's **frame** (equal
    /// angle coefficient `c` and equal `ρ²` — the support-independent pair that pins `ρ`, `ψ`).
    pub fn new(regions: Vec<(Interval<B>, ConeDevelopment<B>)>) -> Option<Self> {
        let (_, first) = regions.first()?;
        for (band, dev) in &regions {
            if band.lo.cmp(&band.hi) != Ordering::Less {
                return None;
            }
            if dev.angle_coeff().cmp(first.angle_coeff()) != Ordering::Equal
                || !dev.rho_sq().sub(first.rho_sq()).is_zero()
            {
                return None;
            }
        }
        for w in regions.windows(2) {
            if w[0].0.hi.cmp(&w[1].0.lo) != Ordering::Equal {
                return None;
            }
        }
        Some(Self {
            regions,
            cum_cache: RefCell::new(None),
        })
    }

    /// The glued regions `(σ-band, development)` in σ order — the piecewise fold's iteration
    /// surface (each region is inverted in its own running frame).
    pub(crate) fn regions(&self) -> &[(Interval<B>, ConeDevelopment<B>)] {
        &self.regions
    }

    /// The glued σ-domain `[first.lo, last.hi]`.
    pub fn span(&self) -> Interval<B> {
        Interval {
            lo: self.regions[0].0.lo.clone(),
            hi: self.regions[self.regions.len() - 1].0.hi.clone(),
        }
    }

    /// The index of the region whose window contains σ — a **join goes to the earlier region**
    /// (the demo convention; the cumulative γ makes both assignments agree). `None` outside the
    /// glued domain.
    fn region_of(&self, sigma: &Rat<B>) -> Option<usize> {
        self.regions.iter().position(|(band, _)| {
            band.lo.cmp(sigma) != Ordering::Greater && sigma.cmp(&band.hi) != Ordering::Greater
        })
    }

    /// The cumulative γ over the full windows of the regions **before** `k` (memoized — see
    /// [`PiecewiseDevelopment::cum_cache`]).
    pub(crate) fn cum_before(&self, k: usize, cfg: &DevConfig<B>) -> Option<[RatIv<B>; 2]> {
        {
            let cache = self.cum_cache.borrow();
            if let Some(c) = cache.as_ref()
                && c.terms == cfg.terms
                && c.sqrt_eps.cmp(&cfg.sqrt_eps) == Ordering::Equal
            {
                return Some(c.cum[k].clone());
            }
        }
        let zero = RatIv::point(Rat::from_i128(0));
        let mut acc = [zero.clone(), zero];
        let mut cum = Vec::with_capacity(self.regions.len() + 1);
        cum.push(acc.clone());
        for (band, dev) in &self.regions {
            let g = dev.directrix_between(&band.lo, &band.hi, cfg)?;
            acc = [acc[0].add(&g[0]).rounded(), acc[1].add(&g[1]).rounded()];
            cum.push(acc.clone());
        }
        let out = cum[k].clone();
        *self.cum_cache.borrow_mut() = Some(CumCache {
            terms: cfg.terms,
            sqrt_eps: cfg.sqrt_eps.clone(),
            cum,
        });
        Some(out)
    }

    /// The **cumulative running directrix** `γ(σ)`: the full-window γ of every region before the
    /// one containing σ, plus that region's own γ from its window start — each region integrates
    /// only where its support is tame. `None` outside the domain or on a pole.
    pub fn gamma_at(&self, sigma: &Rat<B>, cfg: &DevConfig<B>) -> Option<[RatIv<B>; 2]> {
        let k = self.region_of(sigma)?;
        let acc = self.cum_before(k, cfg)?;
        let (band, dev) = &self.regions[k];
        let g = dev.directrix_between(&band.lo, sigma, cfg)?;
        Some([acc[0].add(&g[0]).rounded(), acc[1].add(&g[1]).rounded()])
    }
}

/// The larger of two rationals (by [`Rat::cmp`]).
fn rat_max<B: Backend>(a: &Rat<B>, b: &Rat<B>) -> Rat<B> {
    if a.cmp(b) == Ordering::Less {
        b.clone()
    } else {
        a.clone()
    }
}

/// The smaller of two rationals (by [`Rat::cmp`]).
fn rat_min<B: Backend>(a: &Rat<B>, b: &Rat<B>) -> Rat<B> {
    if a.cmp(b) == Ordering::Greater {
        b.clone()
    } else {
        a.clone()
    }
}

impl<B: Backend> Development<B> for PiecewiseDevelopment<B> {
    /// The connected flat point: the region containing σ develops `base + ∫ γ′ + µ̂·ρ·e(ψ)` from
    /// its own window start on the running base — **signed** µ̂ (the canonical development), so
    /// all regions land in one continuous flat frame.
    fn point(&self, sigma: &Rat<B>, mu_hat: &Rat<B>, cfg: &DevConfig<B>) -> Option<FlatBox<B>> {
        let k = self.region_of(sigma)?;
        let base = self.cum_before(k, cfg)?;
        let (band, dev) = &self.regions[k];
        dev.point_from(&base, &band.lo, sigma, mu_hat, cfg)
    }

    fn point_on(
        &self,
        sigma: &RatIv<B>,
        mu_hat: &RatIv<B>,
        cfg: &DevConfig<B>,
    ) -> Option<FlatBox<B>> {
        // Split the σ-interval at region joins and hull the per-region boxes: each sub-interval
        // develops through its own region's window (from-`lo` γ on the running base).
        let (k_lo, k_hi) = (self.region_of(sigma.lo())?, self.region_of(sigma.hi())?);
        let mut acc: Option<FlatBox<B>> = None;
        for k in k_lo..=k_hi {
            let (band, dev) = &self.regions[k];
            let lo = rat_max(&band.lo, sigma.lo());
            let hi = rat_min(&band.hi, sigma.hi());
            let base = self.gamma_at(&lo, cfg)?;
            let fb = dev.point_from_on(&base, &lo, &RatIv::new(lo.clone(), hi), mu_hat, cfg)?;
            acc = Some(match acc {
                None => fb,
                Some(prev) => FlatBox {
                    x: prev.x.hull_with(&fb.x),
                    y: prev.y.hull_with(&fb.y),
                },
            });
        }
        acc
    }

    fn angle_on(&self, sigma: &RatIv<B>, terms: usize) -> RatIv<B> {
        // ψ is support-independent — any region's frame serves.
        self.regions[0].1.angle_on(sigma, terms)
    }

    fn radius_on(&self, sigma: &RatIv<B>, eps: &Rat<B>) -> Option<RatIv<B>> {
        // ρ is support-independent — any region's frame serves.
        self.regions[0].1.radius_on(sigma, eps)
    }

    fn has_directrix(&self) -> bool {
        self.regions.iter().any(|(_, dev)| dev.has_directrix())
    }

    fn anchor_pieces(
        &self,
        span: &Interval<B>,
        cfg: &DevConfig<B>,
    ) -> Option<Vec<AnchorPiece<'_, B>>> {
        // Refuse spans escaping the glued domain (an authoring error, not a refinable enclosure).
        let dom = self.span();
        if span.lo.cmp(&dom.lo) == Ordering::Less || dom.hi.cmp(&span.hi) == Ordering::Less {
            return None;
        }
        let mut pieces = Vec::new();
        for (band, dev) in &self.regions {
            let lo = rat_max(&band.lo, &span.lo);
            let hi = rat_min(&band.hi, &span.hi);
            if lo.cmp(&hi) != Ordering::Less {
                continue; // no (nondegenerate) overlap with this region
            }
            let frame = AnchorFrame {
                base: self.gamma_at(&lo, cfg)?,
                lo: lo.clone(),
            };
            pieces.push(AnchorPiece {
                span: Interval { lo, hi },
                dev,
                frame: Some(frame),
            });
        }
        Some(pieces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::{AnchorDevCert, anchor_dev};
    use certify_core::Verdict;
    use fixtures::devices::{cone, cone_seam_ramp, cone_wrap};
    use lattice::{Poly, RatFunc};

    type Q = Rat<Bignum>;

    fn q(n: i128, d: i128) -> Q {
        Q::new(n, d)
    }
    fn band(lo: Q, hi: Q) -> Interval<Bignum> {
        Interval { lo, hi }
    }
    fn wrap_dev() -> ConeDevelopment<Bignum> {
        ConeDevelopment::new(&cone_wrap()).unwrap()
    }
    fn ramp_dev() -> ConeDevelopment<Bignum> {
        ConeDevelopment::new_developable(&cone_seam_ramp(), 32).unwrap()
    }
    fn cone_dev() -> ConeDevelopment<Bignum> {
        ConeDevelopment::new(&cone()).unwrap()
    }
    /// The device cone (γ≡0) on [0, 1/4] glued to the ramp flap (γ≠0) on [1/4, 1/2] — the two
    /// fixtures share the device frame (same Gauss circle).
    fn two_region() -> PiecewiseDevelopment<Bignum> {
        PiecewiseDevelopment::new(vec![
            (band(q(0, 1), q(1, 4)), cone_dev()),
            (band(q(1, 4), q(1, 2)), ramp_dev()),
        ])
        .unwrap()
    }

    #[test]
    fn a_single_region_gluing_is_the_signed_cone() {
        let pw = PiecewiseDevelopment::new(vec![(band(q(0, 1), q(1, 1)), wrap_dev())]).unwrap();
        let cfg = DevConfig::tight();
        let (s, m) = (q(1, 3), Q::from_i128(-1));
        let glued = Development::point(&pw, &s, &m, &cfg).unwrap().center();
        let signed = wrap_dev().point_signed(&s, &m, &cfg).center();
        assert_eq!(glued, signed);
    }

    #[test]
    fn regions_must_tile_and_share_the_frame() {
        // Reversed band.
        assert!(PiecewiseDevelopment::new(vec![(band(q(1, 1), q(0, 1)), wrap_dev())]).is_none());
        // Gap between the bands.
        assert!(
            PiecewiseDevelopment::new(vec![
                (band(q(0, 1), q(1, 4)), cone_dev()),
                (band(q(1, 3), q(1, 2)), ramp_dev()),
            ])
            .is_none()
        );
        // Overlapping bands.
        assert!(
            PiecewiseDevelopment::new(vec![
                (band(q(0, 1), q(1, 3)), cone_dev()),
                (band(q(1, 4), q(1, 2)), ramp_dev()),
            ])
            .is_none()
        );
        // Frame mismatch: the wrap chart's angle coefficient differs from the device cone's.
        assert!(
            PiecewiseDevelopment::new(vec![
                (band(q(0, 1), q(1, 4)), cone_dev()),
                (band(q(1, 4), q(1, 2)), wrap_dev()),
            ])
            .is_none()
        );
        // The two device fixtures do share a frame.
        assert!(
            PiecewiseDevelopment::new(vec![
                (band(q(0, 1), q(1, 4)), cone_dev()),
                (band(q(1, 4), q(1, 2)), ramp_dev()),
            ])
            .is_some()
        );
    }

    #[test]
    fn gamma_accumulates_across_the_join() {
        // Region 0 (γ ≡ 0) contributes nothing, so the cumulative γ inside the ramp region is
        // the ramp's own window integral — the demo's `gamma_grid` invariant.
        let pw = two_region();
        let cfg = DevConfig::tight();
        let s = q(3, 8);
        let glued = pw.gamma_at(&s, &cfg).unwrap();
        let own = ramp_dev().directrix_between(&q(1, 4), &s, &cfg).unwrap();
        for (g, o) in glued.iter().zip(own.iter()) {
            assert!(g.lo().cmp(o.lo()) != core::cmp::Ordering::Greater);
            assert!(o.hi().cmp(g.hi()) != core::cmp::Ordering::Greater);
        }
        // And on the body it is exactly zero.
        let z = pw.gamma_at(&q(1, 8), &cfg).unwrap();
        assert_eq!(z[0].lo().cmp(&Q::from_i128(0)), core::cmp::Ordering::Equal);
        assert_eq!(z[0].hi().cmp(&Q::from_i128(0)), core::cmp::Ordering::Equal);
    }

    #[test]
    fn the_connected_point_agrees_at_the_join() {
        // The join σ develops through region 0 (the earlier-region convention); developing it
        // through region 1's formula (its base + zero-width own-γ) must enclose the same point —
        // the boxes intersect.
        let pw = two_region();
        let cfg = DevConfig::tight();
        let (s, m) = (q(1, 4), q(1, 1));
        let via_body = Development::point(&pw, &s, &m, &cfg).unwrap();
        let base = pw.cum_before(1, &cfg).unwrap();
        let via_ramp = ramp_dev().point_from(&base, &s, &s, &m, &cfg).unwrap();
        let overlap = |a: &RatIv<Bignum>, b: &RatIv<Bignum>| {
            a.lo().cmp(b.hi()) != core::cmp::Ordering::Greater
                && b.lo().cmp(a.hi()) != core::cmp::Ordering::Greater
        };
        assert!(overlap(&via_body.x, &via_ramp.x) && overlap(&via_body.y, &via_ramp.y));
    }

    #[test]
    fn point_on_encloses_the_pointwise_gluing_across_the_join() {
        let pw = two_region();
        let cfg = DevConfig::tight();
        let sig = RatIv::new(q(1, 5), q(3, 8)); // straddles the 1/4 join
        let mu = RatIv::point(q(1, 1));
        let box_iv = Development::point_on(&pw, &sig, &mu, &cfg).unwrap();
        for s in [q(1, 5), q(1, 4), q(3, 8)] {
            let pt = Development::point(&pw, &s, &q(1, 1), &cfg).unwrap();
            assert!(box_iv.x.contains(&pt.x.mid()) && box_iv.y.contains(&pt.y.mid()));
        }
    }

    #[test]
    fn anchor_pieces_split_at_the_join_and_certify() {
        // A span crossing the join splits into one frameless-equivalent piece per region, each
        // carrying its running base + window-start lo — and each certifies under the extended
        // anchor checker (permissive clearance, zero target: ε is the raw sup |D|).
        let pw = two_region();
        let cfg = DevConfig::tight();
        let span = band(q(1, 8), q(3, 8));
        let pieces = pw.anchor_pieces(&span, &cfg).unwrap();
        assert_eq!(pieces.len(), 2);
        let one = RatFunc::new(
            Poly::from_coeffs(vec![Q::from_i128(0), Q::from_i128(1)]),
            Poly::from_coeffs(vec![Q::from_i128(1)]),
        );
        for p in pieces {
            assert!(p.frame.is_some());
            let cert = AnchorDevCert {
                dev: p.dev.clone(),
                sigma: one.clone(), // σ(t) = t
                mu: RatFunc::new(
                    Poly::from_coeffs(vec![Q::from_i128(1)]),
                    Poly::from_coeffs(vec![Q::from_i128(1)]),
                ),
                target: [
                    RatFunc::new(
                        Poly::from_coeffs(vec![Q::from_i128(0)]),
                        Poly::from_coeffs(vec![Q::from_i128(1)]),
                    ),
                    RatFunc::new(
                        Poly::from_coeffs(vec![Q::from_i128(0)]),
                        Poly::from_coeffs(vec![Q::from_i128(1)]),
                    ),
                ],
                span: p.span,
                subdiv: 4,
                clearance: Q::from_i128(1_000_000),
                cfg: cfg.clone(),
                frame: p.frame,
            };
            assert!(matches!(anchor_dev(&cert), Verdict::Verified(_)));
        }
        // A span escaping the glued domain is refused.
        assert!(pw.anchor_pieces(&band(q(1, 8), q(3, 4)), &cfg).is_none());
    }
}
