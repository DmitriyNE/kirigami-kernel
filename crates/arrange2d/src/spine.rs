//! The event-spine driver (M3a Phase 4) — spec §6 steps 1–4, branch priority
//! most-degenerate-first: (1) CARRIER-COINCIDENT first (its result is deferred to
//! the stage-2 1D coincidence lattice, slice 3c — the seam is here); (2) else
//! solve carrier ∩ carrier; (3) interval membership on both edges before any
//! classification; (4) classify the survivors. The untrusted searcher entry
//! (`arrange_events`), returning a `certify_core::Verdict` of the emitted
//! `EventSet` + a replayable [`super::witness`].
