//! Deterministic **work counters** for the geometry pipeline (VV.1).
//!
//! A wall-clock baseline committed to a repository is a flaky gate: it moves with machine speed,
//! thermal state and background load, so it is either too loose to catch a 2× regression or it
//! cries wolf. The regression this exists to catch was not a constant factor anyway — it was a
//! *complexity* change, `N × panels` work where `N + cells` was available (see the OPT.0/OPT.1
//! entries in `docs/engineering-log.md`). Counting the operations catches that exactly, is
//! identical on every machine, and cannot flake.
//!
//! Counters are **thread-local**, so tests running in parallel do not perturb each other's counts,
//! and always compiled in: a `Cell<u64>` increment is free beside the exact-rational interval
//! arithmetic it sits next to.
//!
//! ```
//! use develop::counters;
//! use develop::cone::{ConeDevelopment, DevConfig};
//! use fixtures::devices::cone_seam_ramp;
//! use lattice::{Bignum, Rat};
//!
//! let dev = ConeDevelopment::<Bignum>::new_developable(&cone_seam_ramp(), 8).unwrap();
//! let cfg = DevConfig::tight();
//! counters::reset();
//! // The first γ query builds the grid; the second is served from it.
//! dev.directrix_between(&Rat::from_i128(0), &Rat::new(1, 2), &cfg).unwrap();
//! let after_first = counters::gamma_cells();
//! dev.directrix_between(&Rat::from_i128(0), &Rat::new(1, 4), &cfg).unwrap();
//! assert_eq!(counters::gamma_cells(), after_first, "the second query reuses the grid");
//! ```

use core::cell::Cell;

thread_local! {
    static GAMMA_CELLS: Cell<u64> = const { Cell::new(0) };
    static CUT_EVALS: Cell<u64> = const { Cell::new(0) };
}

/// Zero every counter on the calling thread. Call at the start of a measurement.
pub fn reset() {
    GAMMA_CELLS.with(|c| c.set(0));
    CUT_EVALS.with(|c| c.set(0));
}

/// Cells of the flat directrix `γ` integrated on this thread — grid cells plus partial cells.
///
/// This is the number the p-curve milestone silently multiplied and OPT.1 collapsed. It measures
/// the *real* work of a curved-support development, and does not depend on how fast the machine
/// runs it.
pub fn gamma_cells() -> u64 {
    GAMMA_CELLS.with(|c| c.get())
}

/// Sub-interval evaluations performed by the p-curve cut certificate on this thread.
///
/// Each one encloses the chart's three vector fields and the cut surface's distance, so this
/// tracks the second hot path — the one whose per-evaluation cost is itself a standing item (the
/// unbounded digit growth noted in `docs/engineering-log.md`).
pub fn cut_evals() -> u64 {
    CUT_EVALS.with(|c| c.get())
}

pub(crate) fn bump_gamma_cell() {
    GAMMA_CELLS.with(|c| c.set(c.get().saturating_add(1)));
}

pub(crate) fn bump_cut_eval() {
    CUT_EVALS.with(|c| c.set(c.get().saturating_add(1)));
}
