//! The replayable `(claim, certificate)` a searcher emits (M3a Phase 4). Per
//! event: the branch taken, the vanishing minors / discriminant `Δ`, the
//! membership comparisons, and the tangency-identity value — everything the
//! future `certify_core::arrange` checker (M3e) needs to re-verify *without*
//! re-searching. Designed and populated now; validated by differential + property
//! + corpus until the checker lands.
