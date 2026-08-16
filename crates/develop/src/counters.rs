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
    static GAMMA_VELOCITY: Cell<u64> = const { Cell::new(0) };
    static CUT_EVALS: Cell<u64> = const { Cell::new(0) };
    static BRACKET_SEEDED: Cell<u64> = const { Cell::new(0) };
    static BRACKET_BISECTED: Cell<u64> = const { Cell::new(0) };
    static POLY_SLICE_CLIPS: Cell<u64> = const { Cell::new(0) };
}

/// Zero every counter on the calling thread. Call at the start of a measurement.
pub fn reset() {
    GAMMA_CELLS.with(|c| c.set(0));
    GAMMA_VELOCITY.with(|c| c.set(0));
    CUT_EVALS.with(|c| c.set(0));
    BRACKET_SEEDED.with(|c| c.set(0));
    BRACKET_BISECTED.with(|c| c.set(0));
    POLY_SLICE_CLIPS.with(|c| c.set(0));
}

/// Cells of the flat directrix `γ` integrated on this thread — grid cells plus partial cells.
///
/// This is the number the p-curve milestone silently multiplied and OPT.1 collapsed. It measures
/// the *real* work of a curved-support development, and does not depend on how fast the machine
/// runs it.
pub fn gamma_cells() -> u64 {
    GAMMA_CELLS.with(|c| c.get())
}

/// Evaluations of the directrix **velocity** `γ′` on this thread — the integrand itself, enclosed
/// over an interval.
///
/// Counted separately from [`gamma_cells`] because it is a **different operation with a different
/// caller**, and conflating them would have hidden a real gap. `gamma_cells` counts the quadrature
/// grid; `directrix_velocity` is called once per *interval* γ query — every lift bound in the
/// unroll issues them — and was previously counted by nothing at all. Measured on the acceptance
/// device: 2 256 cells against **4 896 velocity evaluations**, so the work budget was watching
/// about a third of the γ integrand work and a change that doubled the rest would have passed the
/// gate untouched.
///
/// Kept as its own counter rather than folded into `gamma_cells` so the committed 2 256 baseline
/// keeps meaning what it says.
pub fn gamma_velocity() -> u64 {
    GAMMA_VELOCITY.with(|c| c.get())
}

/// Bump the `γ′` evaluation counter.
pub(crate) fn bump_gamma_velocity() {
    GAMMA_VELOCITY.with(|c| c.set(c.get() + 1));
}

/// Sub-interval evaluations performed by the p-curve cut certificate on this thread.
///
/// Each one encloses the chart's three vector fields and the cut surface's distance, so this
/// tracks the second hot path — the one whose per-evaluation cost is itself a standing item (the
/// unbounded digit growth noted in `docs/engineering-log.md`).
pub fn cut_evals() -> u64 {
    CUT_EVALS.with(|c| c.get())
}

/// σ-inversions served by a **seeded, verified bracket** (MAP.1) on this thread.
///
/// Paired with [`bracket_bisected`] this is the fast path's hit rate. It matters because a seed
/// that silently stops working is invisible in the certificates: the bisection fallback produces
/// the same answer, only slowly — so equal ε proves nothing on its own.
pub fn bracket_seeded() -> u64 {
    BRACKET_SEEDED.with(|c| c.get())
}

/// σ-inversions that fell back to the bisection on this thread — see [`bracket_seeded`].
pub fn bracket_bisected() -> u64 {
    BRACKET_BISECTED.with(|c| c.get())
}

/// σ-slices whose lid the solid builder trimmed through its **general polygon channel** on this
/// thread — one per slice a `(σ, µ̂)` polygon hole reaches (AUTH.2e).
///
/// This one is not a budget, it is a **witness**: with a single polygon hole in the part, a count
/// of 1 says the hole sat inside one slice and a count above 1 says it crossed a σ-station, which
/// is the case AUTH.2e/2 exists for. Nothing else distinguishes them — a hole that crossed a
/// station and one that did not certify alike, build alike, and differ only in which branch of the
/// builder ran. An acceptance demo claiming to exercise the station-crossing path needs to be able
/// to say so, and before this it could only assert consequences that a within-slice hole shares.
pub fn poly_slice_clips() -> u64 {
    POLY_SLICE_CLIPS.with(|c| c.get())
}

/// Bump [`poly_slice_clips`].
///
/// Public, unlike its siblings, because the only caller is one crate up — `export`'s per-slice
/// clipper. The counter still belongs here: this is where the pipeline's measured work is counted
/// and where the single [`reset`] lives, and a second counter module elsewhere would silently make
/// `reset` mean "some of the counters".
pub fn bump_poly_slice_clip() {
    POLY_SLICE_CLIPS.with(|c| c.set(c.get().saturating_add(1)));
}

pub(crate) fn bump_bracket_seeded() {
    BRACKET_SEEDED.with(|c| c.set(c.get().saturating_add(1)));
}

pub(crate) fn bump_bracket_bisected() {
    BRACKET_BISECTED.with(|c| c.set(c.get().saturating_add(1)));
}

pub(crate) fn bump_gamma_cell() {
    GAMMA_CELLS.with(|c| c.set(c.get().saturating_add(1)));
}

pub(crate) fn bump_cut_eval() {
    CUT_EVALS.with(|c| c.set(c.get().saturating_add(1)));
}
