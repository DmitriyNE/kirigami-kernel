//! Exact line predicates (M3a Phase 1). `PARALLEL` := `a_A·b_B − a_B·b_A = 0`
//! (the direction cross, one ring op); `COINCIDENT` := all three 2×2 minors of
//! the stacked `(a, b, c)` rows vanish (kept in three-minor form — it cannot be
//! half-read; normal-pair proportionality alone is WRONG). Plus circle
//! carrier-coincidence (equal center ∧ equal `r²`). Corpus: `cx_parallel_distinct_lines`.
