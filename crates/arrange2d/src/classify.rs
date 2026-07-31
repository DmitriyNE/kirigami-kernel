//! Transverse/tangent classification (M3a Phase 4) under the most-degenerate-first
//! guard `d² > 0 ∨ ¬COINCIDENT`. Tangency by exact A-identity (line/circle
//! `dist² = r²`; circle/circle `d² = (r₁ ± r²)²`), transversality by the sign of
//! `det(ċ_A, ċ_B)`. Emits touch vertices with sidedness bits (raw crossing data;
//! the ℤ₂² face-flip encoding is deferred to slice 3d).
