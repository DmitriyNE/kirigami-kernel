//! Per-edge interval membership (M3a Phase 3), checked on **both** edges *before*
//! any classification; non-members are discarded with no vertex and no record.
//! Winding-aware, but after canonical decomposition it collapses to an x-range +
//! half test (the graph-of-a-function property). This is what discards the
//! pole-adjacent phantom. Corpus: `cx_tangent_outside_arc`.
