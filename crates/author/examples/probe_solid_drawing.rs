//! Probe: the device drawing's `solid()` through the flank-splice chords (#294 → #290).
//!
//! The quickest reproduction of the drawing-as-inner-cut device (ramp moved off the tab, the
//! #294 configuration). Measured 2026-08-18: `VERIFIED, 74 faces, 0 free edges, 129 s` — the
//! splice chords sweep through `brep_trim_solid_regions` into a watertight solid. Not pinned as a
//! test yet; #290's acceptance owns that, together with the pinned device.

use certify_core::Verdict;
use lattice::{Bignum, Rat};

type Q = Rat<Bignum>;

fn main() {
    let mut spec = acceptance::self_lapping_spec();
    spec.ccw.ramp_start = acceptance::lapped::Azimuth::Sigma(Q::new(1, 10));
    spec.ccw.ramp_end = acceptance::lapped::Azimuth::Sigma(Q::new(1, 2));
    spec.inner_profile = Some(acceptance::inner_cut_profile());
    let part = acceptance::self_lapping_cone_from(&spec, 8, 8, false, None);
    let clock = std::time::Instant::now();
    match part.solid() {
        Verdict::Verified(s) => {
            let brep = s.brep();
            println!(
                "solid VERIFIED: {} faces, {} free edges, {:.0}s",
                brep.faces().len(),
                brep.free_edges(),
                clock.elapsed().as_secs_f64()
            );
        }
        Verdict::Unresolved(e) => {
            let (n, d) = e.numer_denom_decimal();
            let fl = n.parse::<f64>().unwrap_or(f64::NAN) / d.parse::<f64>().unwrap_or(f64::NAN);
            println!(
                "solid UNRESOLVED({fl:.4e}), {:.0}s",
                clock.elapsed().as_secs_f64()
            );
        }
        Verdict::Refuted(f) => {
            println!(
                "solid REFUTED({f:?}), {:.0}s",
                clock.elapsed().as_secs_f64()
            );
        }
    }
}
