//! The `Event` / `EventSet` the spine emits (M3a Phase 4): a touch vertex with
//! its kind, sidedness bits, and provenance. The set is deduped by exact
//! `geom::content::Point2` equality — the ℓ=0 vertex identity (free,
//! classifier-internal); `0 < ℓ < q_sep` edges are never merged.
